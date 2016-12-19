// -*-_-*-_-*-_-*-_-*-
use std::num;
use std::thread;
use std::cmp::min;
use std::env::args;
use std::path::Path;
use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use std::io::Write;
use std::fs::OpenOptions;
use std::time::Duration;
use std::collections::HashMap;

const DEFAULT_DELAY_MS: u64 = 200;
const ERR_WRITE_STDERR: &'static str = "Could not write to stderr";

struct Config {
    delay: u64,
    layout: Layout,
    dest: Box<Write>,
    err: Box<Write>,
}

struct Layout {
    map: HashMap<char, (u8, u8)>,
}

enum LayoutError {
    Empty,
    ReadError(std::io::Error),
    BadKeyId(u32),
    MissingKeyId(u32),
    BadModifier(u32)
}

fn load_layout (layoutfile: &File) -> Result<Layout, LayoutError> {
    let reader = BufReader::new(layoutfile);
    let mut lines = reader.lines();
    if lines.next().is_none() {
        return Err(LayoutError::Empty)
    }

    let mut ln = 1;
    let mut all_kc = HashMap::new();
    for line in lines {
        let mut line = line.map_err(LayoutError::ReadError)?.replace("0x", "");
        if line.len() == 0 {
            continue;
        }
        let cp = line.remove(0);

        let mut split = line.split_whitespace();
        let id = u8::from_str_radix(&split.next().ok_or(LayoutError::MissingKeyId(ln))?, 16)
                    .map_err(|_| LayoutError::BadKeyId(ln))?;
        let md = u8::from_str_radix(&split.next().unwrap_or("00"), 16)
                    .map_err(|_| LayoutError::BadModifier(ln))?;

        all_kc.insert(cp, (id, md));
        ln += 1;
    }

    Ok(Layout { map: all_kc })
}

// ghetto lookup table
fn lookup_escape (name: &str) -> Option<(u8, u8)> {
    match name {
        "ALT"       => Some((0x00, 0x04)),
        "BACKSPACE" => Some((0x2A, 0x00)),
        "DELETE"    => Some((0x4C, 0x00)),
        "ESCAPE"    => Some((0x29, 0x00)),
        "END"       => Some((0x4D, 0x00)),
        "HOME"      => Some((0x4A, 0x00)),
        "INSERT"    => Some((0x49, 0x00)),
        "ENTER"     => Some((0x28, 0x00)),
        "SPACE"     => Some((0x2C, 0x00)),
        "PRNTSCRN"  => Some((0x46, 0x00)),
        "SCRLLCK"   => Some((0x47, 0x00)),
        "MENU"      => Some((0x76, 0x00)),
        "SHIFT"     => Some((0x00, 0x02)),
        "TAB"       => Some((0x2B, 0x00)),
        "CAPSLOCK"  => Some((0x39, 0x00)),
        "PAUSE"     => Some((0x48, 0x00)),
        "NUMLOCK"   => Some((0x53, 0x00)),
        "PAGEDOWN"  => Some((0x4E, 0x00)),
        "PAGEUP"    => Some((0x4B, 0x00)),
        "CLEAR"     => Some((0x9C, 0x00)),
        "F1"        => Some((0x3A, 0x00)),
        "F2"        => Some((0x3B, 0x00)),
        "F3"        => Some((0x3C, 0x00)),
        "F4"        => Some((0x3D, 0x00)),
        "F5"        => Some((0x3E, 0x00)),
        "F6"        => Some((0x3F, 0x00)),
        "F7"        => Some((0x40, 0x00)),
        "F8"        => Some((0x41, 0x00)),
        "F9"        => Some((0x42, 0x00)),
        "F10"       => Some((0x43, 0x00)),
        "F11"       => Some((0x44, 0x00)),
        "F12"       => Some((0x45, 0x00)),
        "DOWNARROW" | "DARROW" | "DOWN"
            => Some((0x51, 0x00)),
        "UPARROW" | "UARROW" | "UP"
            => Some((0x52, 0x00)),
        "LEFTARROW" | "LARROW" | "LEFT"
            => Some((0x50, 0x00)),
        "RIGHTARROW" | "RARROW" | "RIGHT"
            => Some((0x4F, 0x00)),
        "CONTROL" | "CTRL"
            => Some((0x00, 0x01)),
        "GUI" | "WINDOWS" | "WIN" | "SUPER" | "COMMAND"
            => Some((0x00, 0x08)),
        _ => None
    }
}

enum CharOrKc {
    Char(char),
    Kc((u8, u8))
}

enum ExecError {
    Incomplete,
    Parse(num::ParseIntError),
    UnknownToken(String),
    NoMapping(char),
    Write(std::io::Error),
}

fn make_hid_report (layout: &Layout, send: &Vec<CharOrKc>) -> Result<[u8; 8], ExecError>  {
    let mut report = [0; 8];
    for i in 0..min(6, send.len()) {
        let kc: (u8, u8) = match send[i] {
            CharOrKc::Char(c) => {
                if c as u32 == 0 { continue; }
                *layout.map.get(&c).ok_or(ExecError::NoMapping(c))?
            },
            CharOrKc::Kc(k) => k
        };
        report[i+2] |= kc.0;
        report[0]   |= kc.1;
    }
    Ok(report)
}

fn exec_line (line: &str, conf: &mut Config) -> Result<(), ExecError> {
    let line = String::from(line);
    let mut tokens = line.split_whitespace();

    let first = tokens.next().unwrap_or("#");

    match first {
        "#" | "REM" => return Ok(()),
        "DEFAULT_DELAY" => {
            conf.delay = tokens.next().unwrap_or(&DEFAULT_DELAY_MS.to_string()).parse().map_err(ExecError::Parse)?;
        },
        "STRING" => {
            let rest = tokens.fold(String::new(), |mut b, m| { b.push_str(m); b });
            let mut chunk = vec![];
            for c in rest.chars() {
                chunk.push(CharOrKc::Char(c));
                let report = make_hid_report(&conf.layout, &chunk)?;
                conf.dest.write_all(&report).map_err(ExecError::Write)?;
                chunk.clear();
                // send empty report
                conf.dest.write_all(&make_hid_report(&conf.layout, &chunk)?).map_err(ExecError::Write)?;
            }
            thread::sleep(Duration::from_millis(conf.delay));
        },
        "DELAY" => {
            let delay: u64 = tokens.next().ok_or(ExecError::Incomplete)?.parse().map_err(ExecError::Parse)?;
            thread::sleep(Duration::from_millis(delay));
        },
        "SIMUL" => {
            let mut chunk = vec![];
            for token in tokens {
                match token.len() {
                    1 => chunk.push(CharOrKc::Char(token.chars().next().unwrap())),
                    _ => chunk.push(CharOrKc::Kc(lookup_escape(&token).ok_or(ExecError::UnknownToken(String::from(token)))?))
                }
            };
            let report = make_hid_report(&conf.layout, &chunk)?;
            conf.dest.write_all(&report).map_err(ExecError::Write)?;
            chunk.clear();
            // send empty report
            conf.dest.write_all(&make_hid_report(&conf.layout, &chunk)?).map_err(ExecError::Write)?;
            thread::sleep(Duration::from_millis(conf.delay));
        },
        "ECHO" => {
            let rest = tokens.fold(String::new(), |mut b, m| { b.push_str(m); b });
            writeln!(&mut conf.err, "{}", rest).map_err(ExecError::Write)?;
            thread::sleep(Duration::from_millis(conf.delay));
        }
        _ => {
            let mut chunk = vec![CharOrKc::Kc(lookup_escape(&first).ok_or(ExecError::UnknownToken(String::from(first)))?)];
            let report = make_hid_report(&conf.layout, &chunk)?;
            conf.dest.write_all(&report).map_err(ExecError::Write)?;
            chunk.clear();
            // send empty report
            conf.dest.write_all(&make_hid_report(&conf.layout, &chunk)?).map_err(ExecError::Write)?;
            thread::sleep(Duration::from_millis(conf.delay));
        }
    };
    Ok(())
}

fn main() {
    let ar: Vec<_> = args().collect();
    let mut stderr: Box<Write> = Box::new(std::io::stderr());

    let usage = format!("usage: {} <layout> <script> [output]", ar[0]);

    if ar.len() < 3 {
        writeln!(&mut stderr, "{}", usage).expect(ERR_WRITE_STDERR);
        return ()
    }

    // load layout file
    let mut path = Path::new(&ar[1]);
    let lf = match File::open(&path) {
        Ok(f) => f,
        Err(e) => {
            writeln!(stderr, "{}", e.to_string()).expect(ERR_WRITE_STDERR);
            return ()
        }
    };

    let layout = match load_layout(&lf) {
        Ok(l) => l,
        Err(e) => {
            match e {
                LayoutError::Empty => writeln!(stderr, "Layout file is empty"),
                LayoutError::ReadError(e) => writeln!(stderr, "Error reading layout file: {}", e.to_string()),
                LayoutError::BadKeyId(l) => writeln!(stderr, "{}: Unintelligible key id", l),
                LayoutError::MissingKeyId(l) => writeln!(stderr, "{}: No key ID", l),
                LayoutError::BadModifier(l) => writeln!(stderr, "{}: Bad modifier byte", l)
            }.expect(ERR_WRITE_STDERR);
            return ()
        }
    };

    // load script file
    path = Path::new(&ar[2]);
    let sf = match File::open(&path) {
        Ok(file) => file,
        Err(e) => {
            writeln!(stderr, "{}", e).expect(ERR_WRITE_STDERR);
            return ()
        }
    };

    let output: Box<Write> = match ar.len() {
        4 => {
            // load output file
            path = Path::new(&ar[3]);
            Box::new(match OpenOptions::new().write(true).create(true).open(&path) {
                Ok(file) => file,
                Err(e) => {
                    writeln!(stderr, "{}", e).expect(ERR_WRITE_STDERR);
                    return ();
                }
            })
        },
        _ => Box::new(std::io::stdout())
    };


    // make config
    let mut conf = Config { delay: DEFAULT_DELAY_MS, layout: layout, dest: output, err: stderr };

    // REPL
    let mut ln = 1;
    let reader = BufReader::new(sf);
    for line in reader.lines() {
        let aline = match line {
            Ok(l) => l,
            Err(e) => {
                writeln!(conf.err, "{}", e.to_string()).expect(ERR_WRITE_STDERR);
                return ()
            }
        };

        match exec_line (&aline, &mut conf) {
            Err(e) => match e {
                ExecError::Incomplete => writeln!(conf.err, "{}: Incomplete line: {}", ln, aline),
                ExecError::Parse(e) => writeln!(conf.err, "{}: Parse error: {}", ln, e.to_string()),
                ExecError::UnknownToken(t) => writeln!(conf.err, "{}: Unintelligible keyword: {}", ln, &t),
                ExecError::NoMapping(c) => writeln!(conf.err, "{}: No mapping for character: {}", ln, c),
                ExecError::Write(w) => writeln!(conf.err, "{}: Error writing HID report: {}", ln, w.to_string()),
            }.expect(ERR_WRITE_STDERR),
            Ok(_) => (),
        };
        ln += 1;
    }
}
