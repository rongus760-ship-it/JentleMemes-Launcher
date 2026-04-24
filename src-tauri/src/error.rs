use serde::{Serialize, Serializer};
use std::error::Error as StdError;
use std::fmt;
use std::io::ErrorKind;

/// Ошибки бэкенда лаунчера: подробный человекочитаемый вывод и цепочка причин.
#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Json(serde_json::Error),
    Reqwest(reqwest::Error),
    Zip(zip::result::ZipError),
    AsyncZip(async_zip::error::ZipError),
    FromUtf8(std::string::FromUtf8Error),
    Join(tokio::task::JoinError),
    Walkdir(walkdir::Error),
    /// Произвольное сообщение (бизнес-логика, валидация, сетевые сбои верхнего уровня).
    Custom(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_summary())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            Error::Json(e) => Some(e),
            Error::Reqwest(e) => Some(e),
            Error::Zip(e) => Some(e),
            Error::AsyncZip(e) => Some(e),
            Error::FromUtf8(e) => Some(e),
            Error::Join(e) => Some(e),
            Error::Walkdir(e) => Some(e),
            Error::Custom(_) => None,
        }
    }
}

impl Error {
    /// Краткая строка для тостов и подписей (1–2 предложения).
    pub fn display_summary(&self) -> String {
        match self {
            Error::Io(e) => format!("Файловая система: {} ({})", e, io_kind_ru(e.kind())),
            Error::Json(e) => format!("Некорректный JSON: {e}"),
            Error::Reqwest(e) => format!("HTTP / сеть (reqwest): {e}"),
            Error::Zip(e) => format!("Архив ZIP: {e}"),
            Error::AsyncZip(e) => format!("Архив (async-zip): {e}"),
            Error::FromUtf8(e) => format!("Текст не в UTF-8: {e}"),
            Error::Join(e) => format!("Фоновая задача завершилась с ошибкой: {e}"),
            Error::Walkdir(e) => format!("Обход каталога: {e}"),
            Error::Custom(s) => s.clone(),
        }
    }

    /// Развёрнутый отчёт: тип, цепочка причин, контекст и подсказки (для логов и отладки).
    pub fn diagnostic_report(&self) -> String {
        let mut out = String::new();
        out.push_str("═══════════════════════════════════════════════════════════════\n");
        out.push_str(" JentleMemes Launcher — диагностический отчёт об ошибке\n");
        out.push_str("═══════════════════════════════════════════════════════════════\n\n");

        let title = match self {
            Error::Io(_) => "Ввод-вывод (диск, права, отсутствующий файл)",
            Error::Json(_) => "Сериализация / разбор JSON",
            Error::Reqwest(_) => "HTTP-клиент (загрузки, API, TLS)",
            Error::Zip(_) | Error::AsyncZip(_) => "Работа с ZIP-архивом",
            Error::FromUtf8(_) => "Кодировка текста (UTF-8)",
            Error::Join(_) => "Асинхронная задача (tokio::spawn / join)",
            Error::Walkdir(_) => "Обход дерева файлов (walkdir)",
            Error::Custom(_) => "Пользовательское / логическое условие",
        };
        out.push_str(&format!("【Категория】 {title}\n\n"));
        out.push_str("【Кратко】\n");
        out.push_str(&format!("  {}\n\n", self.display_summary()));

        match self {
            Error::Io(e) => {
                out.push_str("【Детали IO】\n");
                out.push_str(&format!("  kind (std): {:?}\n", e.kind()));
                out.push_str(&format!("  raw: {e}\n\n"));
                out.push_str(&format!("【Подсказка】 {}\n\n", io_hint_ru(e.kind())));
            }
            Error::Reqwest(e) => {
                if let Some(status) = e.status() {
                    out.push_str(&format!("【HTTP】 Код ответа: {status}\n\n"));
                }
                if e.is_timeout() {
                    out.push_str("【Подсказка】 Таймаут: проверьте сеть, VPN, файрвол и доступность хоста.\n\n");
                } else if e.is_connect() {
                    out.push_str("【Подсказка】 Не удалось установить соединение: DNS, порт, прокси или блокировка.\n\n");
                }
            }
            Error::Json(_) => {
                out.push_str("【Подсказка】 Ответ сервера или локальный файл не соответствует ожидаемой структуре JSON.\n\n");
            }
            Error::Custom(s) => {
                out.push_str("【Текст ошибки (Custom)】\n");
                for line in s.lines() {
                    out.push_str(&format!("  {line}\n"));
                }
                out.push('\n');
            }
            _ => {}
        }

        out.push_str("【Цепочка std::error::Error】\n");
        let chain = self.source_chain_strings();
        if chain.is_empty() {
            out.push_str("  (нет вложенных причин)\n");
        } else {
            for (i, line) in chain.iter().enumerate() {
                out.push_str(&format!("  [{i}] {line}\n"));
            }
        }
        out.push_str("\n═══════════════════════════════════════════════════════════════\n");
        out
    }

    fn source_chain_strings(&self) -> Vec<String> {
        let mut v = Vec::new();
        let mut cur: Option<&dyn StdError> = StdError::source(self);
        while let Some(e) = cur {
            v.push(e.to_string());
            cur = e.source();
        }
        v
    }
}

fn io_kind_ru(k: ErrorKind) -> &'static str {
    match k {
        ErrorKind::NotFound => "файл или каталог не найден",
        ErrorKind::PermissionDenied => "нет прав доступа",
        ErrorKind::ConnectionRefused => "соединение отклонено",
        ErrorKind::ConnectionReset => "соединение сброшено",
        ErrorKind::ConnectionAborted => "соединение прервано",
        ErrorKind::NotConnected => "нет соединения",
        ErrorKind::AddrInUse => "адрес уже занят",
        ErrorKind::AddrNotAvailable => "адрес недоступен",
        ErrorKind::BrokenPipe => "разорван канал (pipe)",
        ErrorKind::AlreadyExists => "уже существует",
        ErrorKind::WouldBlock => "операция заблокировала бы поток",
        ErrorKind::InvalidInput => "некорректные входные данные",
        ErrorKind::InvalidData => "некорректные данные",
        ErrorKind::TimedOut => "таймаут",
        ErrorKind::Interrupted => "прервано сигналом",
        ErrorKind::UnexpectedEof => "неожиданный конец файла",
        ErrorKind::OutOfMemory => "нехватка памяти",
        ErrorKind::Unsupported => "операция не поддерживается",
        _ => "прочая ошибка ввода-вывода",
    }
}

fn io_hint_ru(k: ErrorKind) -> &'static str {
    match k {
        ErrorKind::NotFound => "Проверьте путь к файлу, что сборка установлена и каталог данных лаунчера доступен.",
        ErrorKind::PermissionDenied => "Запустите лаунчер с достаточными правами или смените владельца каталога данных.",
        ErrorKind::AlreadyExists => "Удалите или переименуйте существующий объект либо выберите другой путь.",
        ErrorKind::TimedOut => "Повторите позже или проверьте сеть / диск (медленный носитель, NFS).",
        _ => "Смотрите цепочку причин выше; при повторении приложите этот отчёт к багрепорту.",
    }
}

macro_rules! from_impl {
    ($ty:ty, $variant:ident) => {
        impl From<$ty> for Error {
            fn from(e: $ty) -> Self {
                Error::$variant(e)
            }
        }
    };
}

from_impl!(std::io::Error, Io);
from_impl!(serde_json::Error, Json);
from_impl!(reqwest::Error, Reqwest);
from_impl!(zip::result::ZipError, Zip);
from_impl!(async_zip::error::ZipError, AsyncZip);
from_impl!(std::string::FromUtf8Error, FromUtf8);
from_impl!(tokio::task::JoinError, Join);
from_impl!(walkdir::Error, Walkdir);

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut st = serializer.serialize_struct("Error", 2)?;
        st.serialize_field("message", &self.display_summary())?;
        st.serialize_field("detail", &self.diagnostic_report())?;
        st.end()
    }
}

pub type Result<T> = std::result::Result<T, Error>;
