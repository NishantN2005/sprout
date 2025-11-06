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