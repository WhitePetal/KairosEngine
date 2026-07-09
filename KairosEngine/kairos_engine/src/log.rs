use std::{backtrace::Backtrace, collections::VecDeque};

pub enum LogLevel {
    Info,
    Warning,
    Error,
}

pub struct LogMessage {
    pub level: LogLevel,
    pub message: String,
    pub backtrace: Backtrace,
}

impl LogMessage {
    fn new(level: LogLevel, message: String, backtrace: Backtrace) -> Self {
        Self {
            level,
            message,
            backtrace,
        }
    }
}

pub struct Log {
    logs: VecDeque<LogMessage>,
}

impl Log {
    pub fn new() -> Self {
        Self {
            logs: VecDeque::new(),
        }
    }

    #[track_caller]
    pub fn info(&mut self, message: &str) {
        self.push(LogLevel::Info, message);
    }

    #[track_caller]
    pub fn warning(&mut self, message: &str) {
        self.push(LogLevel::Warning, message);
    }

    #[track_caller]
    pub fn error(&mut self, message: &str) {
        self.push(LogLevel::Error, message);
    }

    #[track_caller]
    fn push(&mut self, level: LogLevel, message: &str) {
        let caller = std::panic::Location::caller();
        // capture() 遵循 RUST_BACKTRACE 环境变量:
        //   RUST_BACKTRACE=0 (默认) → 不捕获，零开销
        //   RUST_BACKTRACE=1        → 捕获完整调用栈
        let backtrace = Backtrace::capture();
        let message = format!(
            "{}:{}:{}\n{}\n{}",
            caller.file(),
            caller.line(),
            caller.column(),
            message,
            backtrace
        );
        // 同步输出到终端 stderr
        match level {
            LogLevel::Info => eprintln!("[INFO] {}", message),
            LogLevel::Warning => eprintln!("[WARN] {}", message),
            LogLevel::Error => eprintln!("[ERROR] {}", message),
        }
        self.logs
            .push_back(LogMessage::new(level, message, backtrace));
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &LogMessage> {
        self.logs.iter()
    }

    #[inline]
    pub fn pop_front(&mut self) -> Option<LogMessage> {
        self.logs.pop_front()
    }
}
