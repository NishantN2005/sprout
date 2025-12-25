#[derive(Clone, Copy, Debug)]
pub enum Builtin{
    Range
}

impl Builtin{
    pub fn from_name(name: &str) -> Option<Self>{
        match name {
            "range" => Some(Builtin::Range),
            _ => None,
        }
    }
}