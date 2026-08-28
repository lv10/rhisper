// ipc.rs - client/daemon transport for rhisper.
//
// Uses a normal filesystem-path Unix datagram socket, placed under
// $XDG_RUNTIME_DIR (a per-login tmpfs that's cleared on logout) with a /tmp
// fallback. A filesystem-path socket (rather than a Linux-only
// abstract-namespace one) keeps this portable to other Unix-likes.

use std::env;
use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::PathBuf;

const SOCKET_NAME: &str = "rhisper.sock";

pub fn socket_path() -> PathBuf {
    if let Ok(dir) = env::var("XDG_RUNTIME_DIR") {
        if !dir.is_empty() {
            return PathBuf::from(dir).join(SOCKET_NAME);
        }
    }
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/rhisper-{uid}.sock"))
}

/// The single-byte (or two-byte, for `Type`) command protocol spoken over
/// the socket between `rhisper` and the `rhispertoold` daemon.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Paste,
    Type(u8),
    Backspace,
    RightAlt,
    LeftAlt,
    LeftCtrl,
    RightCtrl,
    LeftShift,
    RightShift,
    Super,
}

impl Command {
    pub fn encode(self) -> [u8; 2] {
        match self {
            Command::Paste => [b'p', 0],
            Command::Type(c) => [b't', c],
            Command::Backspace => [b'b', 0],
            Command::RightAlt => [b'r', 0],
            Command::LeftAlt => [b'L', 0],
            Command::LeftCtrl => [b'C', 0],
            Command::RightCtrl => [b'R', 0],
            Command::LeftShift => [b'S', 0],
            Command::RightShift => [b'T', 0],
            Command::Super => [b'M', 0],
        }
    }

    /// Number of meaningful bytes in `encode()`'s output for this command.
    pub fn encoded_len(self) -> usize {
        match self {
            Command::Type(_) => 2,
            _ => 1,
        }
    }

    pub fn decode(buf: &[u8]) -> Option<Command> {
        match buf {
            [b'p'] => Some(Command::Paste),
            [b't', c] => Some(Command::Type(*c)),
            [b'b'] => Some(Command::Backspace),
            [b'r'] => Some(Command::RightAlt),
            [b'L'] => Some(Command::LeftAlt),
            [b'C'] => Some(Command::LeftCtrl),
            [b'R'] => Some(Command::RightCtrl),
            [b'S'] => Some(Command::LeftShift),
            [b'T'] => Some(Command::RightShift),
            [b'M'] => Some(Command::Super),
            _ => None,
        }
    }
}

/// Binds the daemon's listening socket. If a socket file already exists but
/// nothing is listening on it (a stale leftover from a crashed daemon), it's
/// removed and rebinding is retried once.
pub fn bind_daemon_socket() -> io::Result<UnixDatagram> {
    let path = socket_path();
    match UnixDatagram::bind(&path) {
        Ok(sock) => Ok(sock),
        Err(e) if e.kind() == io::ErrorKind::AddrInUse => {
            let probe = UnixDatagram::unbound()?;
            let stale = match probe.connect(&path) {
                Ok(()) => probe.send(&[]).is_err(),
                Err(_) => true,
            };
            if stale {
                std::fs::remove_file(&path)?;
                UnixDatagram::bind(&path)
            } else {
                Err(e)
            }
        }
        Err(e) => Err(e),
    }
}

/// Connects a client socket to an already-running daemon.
pub fn connect_client() -> io::Result<UnixDatagram> {
    let socket = UnixDatagram::unbound()?;
    socket.connect(socket_path())?;
    Ok(socket)
}

/// A connected client, kept open for a whole `rhisper` invocation instead
/// of reconnecting per command (the original C client connected fresh for
/// every single `rhispertool` subprocess call).
pub struct ToolClient(UnixDatagram);

impl ToolClient {
    pub fn connect() -> io::Result<Self> {
        Ok(ToolClient(connect_client()?))
    }

    fn send(&self, command: Command) -> io::Result<()> {
        let encoded = command.encode();
        self.0.send(&encoded[..command.encoded_len()])?;
        Ok(())
    }

    pub fn paste(&self) -> io::Result<()> {
        self.send(Command::Paste)
    }

    pub fn type_char(&self, c: u8) -> io::Result<()> {
        self.send(Command::Type(c))
    }

    pub fn backspace(&self) -> io::Result<()> {
        self.send(Command::Backspace)
    }

    pub fn press(&self, command: Command) -> io::Result<()> {
        self.send(command)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_roundtrip() {
        let cases = [
            Command::Paste,
            Command::Type(b'x'),
            Command::Backspace,
            Command::RightAlt,
            Command::LeftAlt,
            Command::LeftCtrl,
            Command::RightCtrl,
            Command::LeftShift,
            Command::RightShift,
            Command::Super,
        ];
        for cmd in cases {
            let encoded = cmd.encode();
            let len = cmd.encoded_len();
            assert_eq!(Command::decode(&encoded[..len]), Some(cmd));
        }
    }

    #[test]
    fn command_byte_values_match_original_protocol() {
        assert_eq!(Command::Paste.encode()[0], b'p');
        assert_eq!(Command::Backspace.encode()[0], b'b');
        assert_eq!(Command::RightAlt.encode()[0], b'r');
        assert_eq!(Command::LeftAlt.encode()[0], b'L');
        assert_eq!(Command::LeftCtrl.encode()[0], b'C');
        assert_eq!(Command::RightCtrl.encode()[0], b'R');
        assert_eq!(Command::LeftShift.encode()[0], b'S');
        assert_eq!(Command::RightShift.encode()[0], b'T');
        assert_eq!(Command::Super.encode()[0], b'M');
    }
}
