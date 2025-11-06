
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(u32);



#[derive(Debug, Clone)]
enum Inst { 
    Const {dst: ValueId, value: i64},
    Add {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Sub {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Mul {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Div {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Call {dst: ValueId, func: FuncId, args: Vec<ValueId>},
    Load {name: String, src: ValueId},
    Store {dst: ValueId, name: String},
    Return {src: ValueId},
}

impl ValueId{
    pub fn new(id: u32) -> Self {
        ValueId(id)
    }

    pub fn id(&self) -> u32 {
        self.0
    }
}