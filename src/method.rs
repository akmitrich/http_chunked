#[derive(Debug)]
pub enum Method {
    Get,
    Post,
}

// TODO: consider not to use the AsRef trait
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
