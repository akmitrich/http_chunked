use std::str::FromStr;

/// Only GET & POST methods are acceptable
#[derive(Debug)]
pub enum Method {
    Get,
    Post,
}

impl AsRef<str> for Method {
    fn as_ref(&self) -> &str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

impl ToString for Method {
    fn to_string(&self) -> String {
        self.as_ref().to_owned()
    }
}

impl FromStr for Method {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "GET" => Ok(Self::Get),
            "POST" => Ok(Self::Post),
            _ => Err(anyhow::Error::msg("Unacceptable method")),
        }
    }
}
