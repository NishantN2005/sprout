#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId(u32);

//produce functions
#[derive(Debug)]
pub struct Function{
    pub name: String,
    pub body: Vec<Inst>,
    next_value: u32 // generate new value id's
}

#[derive(Debug)]
pub struct Module{
    pub functions: Vec<Function>,
}

//defines simple instruction set
#[derive(Debug, Clone)]
pub enum Inst { 
    Const {dst: ValueId, value: i64},
    Boolean {dst: ValueId, value: bool},
    Add {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Sub {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Mul {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Div {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Greater {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Less {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Equal {dst: ValueId, lhs: ValueId, rhs: ValueId},
    Call  { dst: ValueId, callee: String, args: Vec<ValueId> },
    Load {dst: ValueId, name: String},
    Store {name: String, src: ValueId},
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

    pub fn dump(&self) {
        println!("fn {}() {{", self.name);
        for (i, inst) in self.body.iter().enumerate() {
            println!("  v{:02}: {:?}", i, inst);
        }
        println!("}}");
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

impl ValueId{
    pub fn from_usize(id: usize) -> Self {
        ValueId(id as u32)
    }
    pub fn get_usize(&self) -> usize {
        self.0 as usize
    }
}