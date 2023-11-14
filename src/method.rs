#[derive(Debug)]
pub enum Method {
    Get,
    Post,
}

impl ToString for Method {
    fn to_string(&self) -> String {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
        .to_owned()
    }
}
