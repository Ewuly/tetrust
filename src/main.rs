mod display;
mod terminal;
mod util;

use display::Display;
use std::fmt::format;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use util::*;
use std::process::{Command, Stdio};


const BOARD_WIDTH: u32 = 10;
const BOARD_HEIGHT: u32 = 20;
const HIDDEN_ROWS: u32 = 2;

enum Key {
    Up,
    Down,
    Left,
    Right,
    Space,
    CtrlC,
    Char(char),
}

enum GameUpdate {
    KeyPress(Key),
    Tick,
    DurationUpdate(u64),
}

// #[derive{PartialEq, Eq}]
enum GameOver {
    TopOut,
    LockOut,
    BlockOut,
}

#[derive(Debug, Copy, Clone)]
struct Point {
    x: i32,
    y: i32,
}

struct Board {
    cells: [[Option<Color>; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize],
}

impl Board {
    pub fn render(&self, display: &mut Display) {
        for y in HIDDEN_ROWS..BOARD_HEIGHT {
            display.set_text("|", 0, y, Color::Red, Color::Black);
            display.set_text("|", BOARD_WIDTH * 2 + 1, y, Color::Red, Color::Black);
        }
        for x in 0..(BOARD_WIDTH * 2 + 1) {
            display.set_text("-", x, BOARD_HEIGHT, Color::Red, Color::Black);
        }
        for row in 0..BOARD_HEIGHT {
            for col in 0..BOARD_WIDTH {
                if let Some(color) = self.cells[row as usize][col as usize] {
                    let c = 1 + (col * 2);
                    display.set_text(" ", c, row, color, color);
                    display.set_text(" ", c + 1, row, color, color);
                }
            }
        }
    }

    pub fn lock_piece(&mut self, piece: &Piece, origin: Point) {
        piece.each_point(&mut |row, col| {
            let x = origin.x + (col as i32);
            let y = origin.y + (row as i32);
            self.cells[y as usize][x as usize] = Some(piece.color);
        });
    }

    pub fn collision_test(&self, piece: &Piece, origin: Point) -> bool {
        let mut found = false;
        piece.each_point(&mut |row, col| {
            if !found {
                let x = origin.x + col;
                let y = origin.y + row;
                if x < 0
                    || x >= (BOARD_WIDTH as i32)
                    || y < 0
                    || y >= (BOARD_HEIGHT as i32)
                    || self.cells[y as usize][x as usize] != None
                {
                    found = true;
                }
            }
        });

        found
    }

    /// Clears the board of any complete lines, shifting down rows to take their place.
    /// Returns the total number of lines that were cleared.
    fn clear_lines(&mut self) -> u32 {
        let mut cleared_lines: usize = 0;
        for row in (0..self.cells.len()).rev() {
            if (row as i32) - (cleared_lines as i32) < 0 {
                break;
            }

            if cleared_lines > 0 {
                self.cells[row] = self.cells[row - cleared_lines];
                self.cells[row - cleared_lines] = [None; BOARD_WIDTH as usize];
            }

            while !self.cells[row].iter().any(|x| *x == None) {
                cleared_lines += 1;
                self.cells[row] = self.cells[row - cleared_lines];
                self.cells[row - cleared_lines] = [None; BOARD_WIDTH as usize];
            }
        }

        cleared_lines as u32
    }
}

struct Piece {
    color: Color,
    shape: Vec<Vec<u8>>,
}

impl Clone for Piece {
    fn clone(&self) -> Piece {
        let mut p = Piece {
            color: self.color,
            shape: Vec::with_capacity(self.shape.len()),
        };
        for row in &self.shape {
            p.shape.push(row.clone());
        }
        p
    }
}

impl Piece {
    pub fn new_o() -> Piece {
        Piece {
            color: Color::Cyan,
            shape: vec![vec![1, 1], vec![1, 1]],
        }
    }

    pub fn new_l() -> Piece {
        Piece {
            color: Color::Orange,
            shape: vec![vec![0, 0, 1], vec![1, 1, 1], vec![0, 0, 0]],
        }
    }

    pub fn new_j() -> Piece {
        Piece {
            color: Color::Blue,
            shape: vec![vec![1, 0, 0], vec![1, 1, 1], vec![0, 0, 0]],
        }
    }

    pub fn new_t() -> Piece {
        Piece {
            color: Color::Purple,
            shape: vec![vec![0, 1, 0], vec![1, 1, 1], vec![0, 0, 0]],
        }
    }

    pub fn new_s() -> Piece {
        Piece {
            color: Color::Green,
            shape: vec![vec![0, 1, 1], vec![1, 1, 0], vec![0, 0, 0]],
        }
    }

    pub fn new_z() -> Piece {
        Piece {
            color: Color::Red,
            shape: vec![vec![1, 1, 0], vec![0, 1, 1], vec![0, 0, 0]],
        }
    }

    pub fn new_i() -> Piece {
        Piece {
            color: Color::Cyan,
            shape: vec![
                vec![0, 0, 0, 0],
                vec![1, 1, 1, 1],
                vec![0, 0, 0, 0],
                vec![0, 0, 0, 0],
            ],
        }
    }

    fn rotate(&mut self, direction: Direction) {
        let size = self.shape.len();

        for row in 0..size / 2 {
            for col in row..(size - row - 1) {
                let t = self.shape[row][col];

                match direction {
                    Direction::Left => {
                        self.shape[row][col] = self.shape[col][size - row - 1];
                        self.shape[col][size - row - 1] =
                            self.shape[size - row - 1][size - col - 1];
                        self.shape[size - row - 1][size - col - 1] =
                            self.shape[size - col - 1][row];
                        self.shape[size - col - 1][row] = t;
                    }
                    Direction::Right => {
                        self.shape[row][col] = self.shape[size - col - 1][row];
                        self.shape[size - col - 1][row] =
                            self.shape[size - row - 1][size - col - 1];
                        self.shape[size - row - 1][size - col - 1] =
                            self.shape[col][size - row - 1];
                        self.shape[col][size - row - 1] = t;
                    }
                }
            }
        }
    }

    fn each_point(&self, callback: &mut dyn FnMut(i32, i32)) {
        let piece_width = self.shape.len() as i32;
        for row in 0..piece_width {
            for col in 0..piece_width {
                if self.shape[row as usize][col as usize] != 0 {
                    callback(row, col);
                }
            }
        }
    }
}

/// Implements a queue of randomized tetrominoes.
///
/// Instead of a purely random stream of tetromino types, this queue generates a random ordering of all
/// possible types and ensures all of those pieces are used before re-generating a new random set. This helps
/// avoid pathological cases where purely random generation provides the same piece type repeately in a row,
/// or fails to provide a required piece for a very long time.
struct PieceBag {
    pieces: Vec<Piece>,
}

impl PieceBag {
    fn new() -> PieceBag {
        let mut p = PieceBag { pieces: Vec::new() };
        p.fill_bag();
        p
    }

    /// Removes and returns the next piece in the queue.
    fn pop(&mut self) -> Piece {
        let piece = self.pieces.remove(0);
        if self.pieces.is_empty() {
            self.fill_bag();
        }
        piece
    }

    /// Returns a copy of the next piece in the queue.
    fn peek(&self) -> Piece {
        match self.pieces.first() {
            Some(p) => p.clone(),
            None => panic!("No next piece in piece bag"),
        }
    }

    /// Generates a random ordering of all possible pieces and adds them to the piece queue.
    fn fill_bag(&mut self) {
        use rand::Rng;

        let mut pieces: Vec<Piece> = vec![
            Piece::new_o(),
            Piece::new_l(),
            Piece::new_j(),
            Piece::new_t(),
            Piece::new_s(),
            Piece::new_z(),
            Piece::new_i(),
        ];

        let mut rng = rand::thread_rng();
        while !pieces.is_empty() {
            let i = rng.gen::<usize>() % pieces.len();
            self.pieces.push(pieces.swap_remove(i));
        }
    }
}

struct Game {
    board: Board,
    piece_bag: PieceBag,
    piece: Piece,
    piece_position: Point,
    score: u32,
    level: u32,
    duration: u64,
}

impl Game {
    fn new() -> Game {
        let mut piece_bag = PieceBag::new();
        let piece = piece_bag.pop();

        let mut game = Game {
            board: Board {
                cells: [[None; BOARD_WIDTH as usize]; BOARD_HEIGHT as usize],
            },
            piece_bag: piece_bag,
            piece: piece,
            piece_position: Point { x: 0, y: 0 },
            score: 0,
            level: 1,
            duration: 0,
        };

        game.place_new_piece();
        game
    }

    /// Returns the new position of the current piece if it were to be dropped.
    fn find_dropped_position(&self) -> Point {
        let mut origin = self.piece_position;
        while !self.board.collision_test(&self.piece, origin) {
            origin.y += 1;
        }
        origin.y -= 1;
        origin
    }

    /// Draws the game to the display.
    fn render(&self, display: &mut Display) {
        // Render the board
        self.board.render(display);

        // Render the level
        let left_margin = BOARD_WIDTH * 2 + 5;
        let level_line = format!("Level: {}", self.level);
        display.set_text(&level_line, left_margin, 3, Color::Red, Color::Black);
        let score_line = format!("Score: {}", self.score);
        display.set_text(&score_line, left_margin, 4, Color::Red, Color::Black);
        let score_line = format!("Speed: {}", self.duration);
        display.set_text(&score_line, left_margin, 5, Color::Red, Color::Black);

        // Render the currently falling piece
        let x = 1 + (2 * self.piece_position.x);
        self.render_piece(
            display,
            &self.piece,
            Point {
                x: x,
                y: self.piece_position.y,
            },
        );

        // Render a ghost piece
        let ghost_position = self.find_dropped_position();
        self.render_piece(
            display,
            &self.piece,
            Point {
                x: x,
                y: ghost_position.y,
            },
        );

        // Render the next piece
        display.set_text("Next piece:", left_margin, 7, Color::Red, Color::Black);
        let next_piece = self.piece_bag.peek();
        self.render_piece(
            display,
            &next_piece,
            Point {
                x: (left_margin as i32) + 2,
                y: 9,
            },
        );
    }

    fn render_piece(&self, display: &mut Display, piece: &Piece, origin: Point) {
        let color = piece.color;

        piece.each_point(&mut |row, col| {
            let x = (origin.x + 2 * col) as u32;
            let y = (origin.y + row) as u32;
            display.set_text(" ", x, y, color, color);
            display.set_text(" ", x + 1, y, color, color);
        });
    }

    /// Moves the current piece in the specified direction. Returns true if the piece could be moved and
    /// didn't collide.
    fn move_piece(&mut self, x: i32, y: i32) -> bool {
        let new_position = Point {
            x: self.piece_position.x + x,
            y: self.piece_position.y + y,
        };
        if self.board.collision_test(&self.piece, new_position) {
            false
        } else {
            self.piece_position = new_position;
            true
        }
    }

    /// Rotates the current piece in the specified direction. Returns true if the piece could be rotated
    /// without any collisions.
    fn rotate_piece(&mut self, direction: Direction) -> bool {
        let mut new_piece = self.piece.clone();
        new_piece.rotate(direction);

        if self.board.collision_test(&new_piece, self.piece_position) {
            false
        } else {
            self.piece = new_piece;
            true
        }
    }

    /// Positions the current piece at the top of the board. Returns true if the piece can be placed without
    /// any collisions.
    fn place_new_piece(&mut self) -> bool {
        let origin = Point {
            x: ((BOARD_WIDTH - (self.piece.shape.len() as u32)) / 2) as i32,
            y: 0,
        };
        if self.board.collision_test(&self.piece, origin) {
            //GAME OVER
            false
        } else {
            self.piece_position = origin;
            true
        }
    }

    /// Advances the game by moving the current piece down one step. If the piece cannot move down, the piece
    /// is locked and the game is set up to drop the next piece.  Returns true if the game could be advanced,
    /// false if the player has lost.
    fn advance_game(&mut self) -> bool {
        if !self.move_piece(0, 1) {
            self.board.lock_piece(&self.piece, self.piece_position);
            let increm = self.board.clear_lines();
            self.score = self.score + increm;
            if self.score % 10 == 0 && self.score != 0 {
                self.level += 1;
            }
            self.piece = self.piece_bag.pop();

            if !self.place_new_piece() {
                return false;
            }
        }

        true
    }

    /// Drops the current piece to the lowest spot on the board where it fits without collisions and
    /// advances the game.
    fn drop_piece(&mut self) -> bool {
        while self.move_piece(0, 1) {}
        self.advance_game()
    }

    fn keypress(&mut self, key: Key) {
        match key {
            Key::Left => self.move_piece(-1, 0),
            Key::Right => self.move_piece(1, 0),
            Key::Down => self.advance_game(),
            Key::Up => self.rotate_piece(Direction::Left),
            Key::Space => self.drop_piece(),
            Key::Char('q') => self.rotate_piece(Direction::Left),
            Key::Char('e') => self.rotate_piece(Direction::Right),
            _ => false,
        };
    }

    fn play(&mut self, display: &mut Display) {
        let (tx_event, rx_event) = mpsc::channel();
        let (tx_duration, rx_duration) = mpsc::channel();
        let mut duration = 200;
        //let (tx_duration)

        // Spawn a thread which sends periodic game ticks to advance the piece
        {
            let tx_event = tx_event.clone();
            thread::spawn(move || {
                loop {
                    // print!("Hello : {}",rx_duration.recv().unwrap());
                    // thread::sleep(Duration::from_millis(rx_duration.recv().unwrap()));
                    thread::sleep(Duration::from_millis(duration));
                    //if let Ok(new_duration )
                    tx_event.send(GameUpdate::Tick).unwrap();
                }
            });
        }

        // Spawn a thread which listens for keyboard input
        {
            let tx_event = tx_event.clone();
            thread::spawn(move || {
                let stdin = &mut std::io::stdin();
                loop {
                    match get_input(stdin) {
                        Some(k) => tx_event.send(GameUpdate::KeyPress(k)).unwrap(),
                        None => (),
                    }
                }
            });
        }

        //thread api
        {
            let tx_event = tx_event.clone();
            thread::spawn(move || {
                let mut previous_price: f64 = 0.0;
                loop{
                    let curl_output = Command::new("curl")
                        .args(&[
                            "-s",
                            "https://api.binance.com/api/v3/ticker/price?symbol=BTCUSDT",
                        ])
                        .stdout(Stdio::piped())
                        .spawn()
                        .expect("Failed to execute curl command");

                    let curl_stdout = curl_output.stdout.expect("Failed to read stdout of curl");

                    let jq_output = Command::new("jq")
                        .args(&["-r", ".price"])
                        .stdin(Stdio::from(curl_stdout))
                        .output()
                        .expect("Failed to execute jq command");

                    let current_price = String::from_utf8_lossy(&jq_output.stdout);
                    let current_price: f64 = current_price
                        .trim()
                        .parse()
                        .expect("Failed to parse price as f64");

                    if previous_price != 0.0 {
                        let price_change = current_price - previous_price;
                        let percentage_change = (price_change / previous_price) * 100.0;

                        // println!("Price Change: {:.6} USD", price_change);
                        // println!("{:.6}", percentage_change);
                        if percentage_change > 0.0 && duration >=500 {
                            
                            duration = duration - 500;
                        } else {
                            duration = duration + 500;
                        }
                    }

                    previous_price = current_price;
                    // println!("Duration: {}", duration);

                    tx_duration.send(duration).unwrap();
                    tx_event.send(GameUpdate::DurationUpdate(duration)).unwrap();
                    thread::sleep(Duration::from_millis(5000));

                    // duration=0;
                }
            });
        }
        


        // Main game loop. The loop listens and responds to timer and keyboard updates received on a channel
        // as sent by the threads spawned above.
        loop {
            display.clear_buffer();
            self.render(display);
            display.render();

            match rx_event.recv() {
                Ok(update) => {
                    match update {
                        GameUpdate::KeyPress(key) => {
                            match key {
                                Key::Char('z') | Key::CtrlC => break,
                                k => {
                                    self.keypress(k);
                                }
                            };
                        }
                        GameUpdate::Tick => {
                            self.advance_game();
                        }
                        GameUpdate::DurationUpdate(new_duration) => {
                            duration = new_duration;
                            self.duration = duration;
                        }
                    };
                }
                Err(err) => panic!("{}", err),
            }
        }
    }
}

fn get_input(stdin: &mut std::io::Stdin) -> Option<Key> {
    use std::io::Read;

    let c = &mut [0u8];
    match stdin.read(c) {
        Ok(_) => {
            match std::str::from_utf8(c) {
                Ok("w") => Some(Key::Up),
                Ok("a") => Some(Key::Left),
                Ok("s") => Some(Key::Down),
                Ok("d") => Some(Key::Right),
                Ok(" ") => Some(Key::Space),
                Ok("\x03") => Some(Key::CtrlC),
                // Escape sequence started - must read two more bytes.
                Ok("\x1b") => {
                    let code = &mut [0u8; 2];
                    match stdin.read(code) {
                        Ok(_) => match std::str::from_utf8(code) {
                            Ok("[A") => Some(Key::Up),
                            Ok("[B") => Some(Key::Down),
                            Ok("[C") => Some(Key::Right),
                            Ok("[D") => Some(Key::Left),
                            _ => None,
                        },
                        Err(msg) => panic!("could not read from standard in: {}", msg),
                    }
                }
                Ok(n) => Some(Key::Char(n.chars().next().unwrap())),
                _ => None,
            }
        }
        Err(msg) => panic!("could not read from standard in: {}", msg),
    }
}

fn main() {
    let display = &mut Display::new(BOARD_WIDTH * 2 + 100, BOARD_HEIGHT + 2);
    let game = &mut Game::new();

    let _restorer = terminal::set_terminal_raw_mode();

    game.play(display);
}
