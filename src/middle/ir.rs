#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(u32);

//produce functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Function{
    pub name: String,
    pub body: Vec<Inst>,
    next_value: u32 // generate new value id's
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Module{
    pub functions: Vec<Funcunction>,
}

//defines simple instruction set
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

//create and getter for ValueId
impl ValueId{
    pub fn new(id: u32) -> Self {
        ValueId(id)
    }

    pub fn id(&self) -> u32 {
        self.0
    }
}

//create and new var id generator for Function
impl Function{
    pub fn new(name: String) -> Self {
        Function {
            name,
            body: Vec::new(),
            next_value: 0,
        }
    }

    pub fn fresh_value(&mut self) -> ValueId {
        let id = ValueId(self.next_value);
        self.next_value += 1;
        id
    }
}

//create and add function for Module
impl Module{
    pub fn new() -> Self{
        Self { functions: Vec::new() }
    }

    pub fn add_function(&mut self, func: Function){
        self.functions.push(func);
    }
}