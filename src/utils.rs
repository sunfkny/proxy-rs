use std::io::{self, Write};

pub fn ask_for_confirmation(prompt: &str) -> bool {
    print!("[QUESTION] {} (y/N) ", prompt);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().eq_ignore_ascii_case("y")
}

pub fn find_unused_port(start_port: u16) -> Option<u16> {
    (start_port..65535).find(|port| std::net::TcpListener::bind(("127.0.0.1", *port)).is_ok())
}
