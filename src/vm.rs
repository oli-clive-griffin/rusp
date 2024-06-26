use crate::builtins_comp::{self, BuiltIn};
use crate::disassembler::disassemble;
use crate::static_stack::StaticStack;

use num_enum::{IntoPrimitive, TryFromPrimitive};

use std::alloc::{alloc, Layout};
use std::collections::HashMap;
use std::default;
use std::fmt::{Debug, Display};

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, Copy, IntoPrimitive, TryFromPrimitive)]
pub enum CaptureType {
    SurroundingUpvalue = 0,
    SurroundingLocal = 1,
}
use CaptureType::*;

// struct LinkedListItem<T> {
//     next: Box<LinkedListItem<T>>,
//     value: T,
// }

const STACK_SIZE: usize = 4096; // will need to dial this in
pub struct VM {
    pub stack: StaticStack<SmallVal, STACK_SIZE>, // pub for testing, ugh
    pub globals: HashMap<String, SmallVal>,       // same, need to make interface nicer
    ip: *const u8,
    callframes: Vec<CallFrame>,
    heap: *mut HeapObject,
    chunk_constants: Vec<ConstantValue>,
    // open_upvalues: *mut UpValue,
    // open_upvalues: *mut ObjectValue,
    open_upvalues: *mut HeapObject,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpValue {
    location: *mut SmallVal,
    closed_val: Option<SmallVal>, // I think option is wrong here
    next: *mut HeapObject,
}
impl UpValue {
    fn new(stack_location: *mut SmallVal) -> Self {
        UpValue {
            location: stack_location,
            next: std::ptr::null_mut(),
            closed_val: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Closure {
    pub f: Function,
    // todo implement runtime checks on upvalues.len() == num_upvalues before access, something like that
    upvalues: Vec<*mut HeapObject>,
    pub num_upvalues: usize,
}
impl Closure {
    pub fn new(f: Function, num_upvalues: usize) -> Self {
        Closure {
            f,
            num_upvalues,
            upvalues: (0..num_upvalues).map(|_| std::ptr::null_mut()).collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// only valid on the heap
pub enum ObjectValue {
    SmallValue(SmallVal),
    String(String),
    Closure(Closure),
    Symbol(String),
    ConsCell(ConsCell),
    BuiltIn(BuiltIn),
    UpValue(UpValue),
}

impl ObjectValue {
    fn truthy(&self) -> bool {
        match self {
            ObjectValue::SmallValue(v) => v.truthy(),
            ObjectValue::String(_) => true,
            // ObjectValue::Function(_) => true,
            ObjectValue::Symbol(_) => true,
            ObjectValue::ConsCell(_) => true,
            ObjectValue::BuiltIn(_) => true,
            ObjectValue::UpValue(_) => unreachable!(),
            ObjectValue::Closure(_) => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConsCell(pub *mut HeapObject, pub *mut HeapObject);

impl ConsCell {
    pub fn new(car: *mut HeapObject, cdr: *mut HeapObject) -> Self {
        ConsCell(car, cdr)
    }
}

impl Display for ConsCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let car = self.0;

        let cdr = if self.1.is_null() {
            "nil".to_string()
        } else {
            // would be nice to actually use a ptr to a Nil value here
            format!("{}", unsafe { &*self.1 })
        };

        write!(f, "({} . {})", unsafe { &*car }, cdr)
    }
}

impl Display for ObjectValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ObjectValue::String(s) => write!(f, "\"{}\"", s),
            ObjectValue::Symbol(s) => write!(f, "{}", s), // might want to add a : here or something
            ObjectValue::ConsCell(cell) => write!(f, "{}", cell),
            ObjectValue::SmallValue(v) => write!(f, "{}", v),
            ObjectValue::BuiltIn(b) => write!(f, "builtin <{}>", b.name),
            ObjectValue::UpValue(u) => write!(f, "{}", &unsafe { &*u.location }),
            ObjectValue::Closure(c) => write!(f, "closure <{}>", c.f.name),
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub arity: usize,
    bytecode: Box<BytecodeChunk>,
    num_locals: usize,
}

impl Debug for Function {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Function(name={}, arity={} bc={})",
            self.name,
            self.arity,
            indent(disassemble(&self.bytecode), 2)
        )
    }
}

impl Function {
    pub fn new(name: String, arity: usize, num_locals: usize, bytecode: BytecodeChunk) -> Self {
        Function {
            name,
            arity,
            num_locals,
            bytecode: Box::new(bytecode),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeapObject {
    next: *mut HeapObject,
    // pub value: *mut ObjectValue,
    pub value: ObjectValue,
    // marked: bool,
}

impl Display for HeapObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.value)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SmallVal {
    Integer(i64),
    Float(f64),
    Bool(bool),
    Nil,
    ObjectPtr(*mut HeapObject),
    Quote(*mut HeapObject),
}

impl Display for SmallVal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SmallVal::Integer(i) => write!(f, "{}", i),
            SmallVal::Float(fl) => write!(f, "{}", fl),
            SmallVal::Bool(b) => write!(f, "{}", b),
            SmallVal::Nil => write!(f, "nil"),
            SmallVal::Quote(c) => write!(f, "'{}", unsafe { &**c }),
            SmallVal::ObjectPtr(ptr) => {
                let val = unsafe { &**ptr };
                write!(f, "{}", val)
            }
        }
    }
}

// trait Pointer {
//     type Target;
//     fn deref(self) -> Self::Target;
// }

// impl Pointer for *mut HeapObject {
//     type Target = ObjectValue;

//     fn deref(self) -> Self::Target {
//         unsafe { &*self }.value
//     }
// }

impl default::Default for SmallVal {
    fn default() -> Self {
        SmallVal::Integer(69) // flag for debugging
    }
}

#[derive(Debug, Clone, PartialEq)]
struct CallFrame {
    closure: Closure,
    return_address: *const u8,
    /// the stack index of the function being called
    start_idx: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstantValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Nil,
    Object(ConstantObject),
    List(Vec<ConstantValue>),
    Quote(Box<ConstantValue>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstantObject {
    String(String),
    Closure(Closure),
    Symbol(String),
}

// impl ConstantObject {
//     fn as_object(&self) -> ObjectValue {
//         match self {
//             ConstantObject::String(s) => ObjectValue::String(s.clone()),
//             ConstantObject::Symbol(s) => ObjectValue::Symbol(s.clone()),
//             ConstantObject::Closure(c) => ObjectValue::Closure(c.clone()),
//         }
//     }
// }

impl Display for ConstantObject {
    /// lot's of duplication going on here, can we define easy mappers between these types?
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConstantObject::String(s) => write!(f, "\"{}\"", s),
            // ConstantObject::Function(func) => write!(f, "function <{}>", func.name),
            ConstantObject::Symbol(s) => write!(f, "{}", s),
            ConstantObject::Closure(c) => write!(f, "closure <{}>", c.f.name),
        }
    }
}

impl SmallVal {
    fn truthy(&self) -> bool {
        match self {
            SmallVal::Integer(_) | SmallVal::Float(_) | SmallVal::Quote(_) => true,
            SmallVal::Nil => false,
            SmallVal::Bool(b) => *b,
            SmallVal::ObjectPtr(ptr) => unsafe { (**ptr).value.truthy() },
        }
    }

    pub fn as_integer(&self) -> Option<&i64> {
        if let Self::Integer(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeChunk {
    pub code: Vec<u8>,
    pub constants: Vec<ConstantValue>,
}

impl BytecodeChunk {
    pub fn new(code: Vec<u8>, constants: Vec<ConstantValue>) -> Self {
        BytecodeChunk { code, constants }
    }
}

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, IntoPrimitive, TryFromPrimitive)]
pub enum Op {
    Constant = 0,
    Add = 1,
    Sub = 2,
    Mul = 3,
    Div = 4,
    GT = 6,
    LT = 7,
    GTE = 8,
    LTE = 9,
    Jump = 10,     // jumps to the specified address
    CondJump = 11, // jumps to the specified address if the top of the stack is not zero
    FuncCall = 12,
    Return = 13,
    DeclareGlobal = 14,
    ReferenceGlobal = 15,
    ReferenceLocal = 16,
    Print = 18,
    // Quote = 19,
    Define = 19,
    ReferenceUpvalue = 20,
    SetUpvalue = 21,
    Closure = 22,
    Pop = 23,
    CloseUpvalue = 24,
    SetLocal = 25,
    DebugEnd = 254,
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    fn new() -> VM {
        let mut vm = VM {
            ip: std::ptr::null_mut(),
            stack: StaticStack::new(),
            heap: std::ptr::null_mut(),
            globals: HashMap::default(),
            callframes: Vec::default(),
            chunk_constants: Vec::default(),
            open_upvalues: std::ptr::null_mut(),
        };

        for builtin in builtins_comp::BUILT_INS.into_iter() {
            let name = builtin.name.to_string();
            let obj_ptr = unsafe { vm.allocate_value(ObjectValue::BuiltIn(builtin.clone())) };
            vm.globals.insert(name, SmallVal::ObjectPtr(obj_ptr));
        }

        vm
    }

    fn gc(&mut self) {
        #[cfg(feature = "gc_debug")]
        println!("gc running");
    }

    fn frame(&self) -> &CallFrame {
        self.callframes.last().expect("expected a call frame")
    }

    pub fn run(&mut self, chunk: BytecodeChunk) {
        // these are kind of like a cache of `chunk`
        // not sure I like this pattern though
        self.ip = chunk.code.as_ptr();
        self.chunk_constants = chunk.constants;

        loop {
            let byte: Op = unsafe { *self.ip }.try_into().unwrap();
            match byte {
                Op::Constant => self.handle_constant(),
                Op::Add => self.handle_add(),
                Op::Sub => self.handle_sub(),
                Op::Mul => self.handle_mul(),
                Op::Div => self.handle_div(),
                Op::GT => self.handle_gt(),
                Op::LT => self.handle_lt(),
                Op::GTE => self.handle_gte(),
                Op::LTE => self.handle_lte(),
                Op::Jump => self.handle_jump(),
                Op::CondJump => self.handle_cond_jump(),
                Op::FuncCall => self.handle_func_call(),
                Op::DeclareGlobal => self.handle_declare_global(),
                Op::ReferenceGlobal => self.handle_reference_global(),
                Op::Print => self.handle_print(),
                Op::ReferenceLocal => self.handle_reference_local(),
                Op::Return => self.handle_return(),
                // Op::Quote => self.handle_quote(),
                Op::Define => self.handle_local_define(),
                Op::DebugEnd => return,
                Op::Closure => self.handle_closure(),
                Op::ReferenceUpvalue => self.handle_reference_upvalue(),
                Op::SetUpvalue => self.handle_set_upvalue(),
                Op::Pop => self.handle_pop(),
                Op::CloseUpvalue => unimplemented!(),
                Op::SetLocal => self.handle_set_local(),
            }
        }
    }

    fn handle_pop(&mut self) {
        self.stack.pop().expect("expected value to pop");
        self.advance();
    }

    fn handle_reference_upvalue(&mut self) {
        let upvalue_idx = self.consume_next_byte_as_byte() as usize;
        let ptr = self.frame().closure.upvalues[upvalue_idx];
        let val = match &unsafe { &*ptr }.value {
            ObjectValue::UpValue(uv) => {
                println!("found upvalue {}", upvalue_idx);
                unsafe { &*uv.location }.clone()
                // ALERT ALERT ALERT           ^^V^^
                // this is wrong ----------------'
                // ALERT ALERT ALERT
                // ALERT ALERT ALERT
            }
            got => panic!("expected upvalue, got {got}"),
        };
        self.stack.push(val); // SmallVal::ObjectPtr(ptr));
        self.advance();
    }

    fn handle_set_upvalue(&mut self) {
        let value = self.stack.pop().expect("expected value");
        let idx = self.consume_next_byte_as_byte() as usize;
        let ptr = self.frame().closure.upvalues[idx];
        match &unsafe { &*ptr }.value {
            ObjectValue::UpValue(uv) => {
                *unsafe { &mut *uv.location } = value;
            }
            _ => panic!("expected upvalue"),
        };
        self.advance();
    }

    fn handle_set_local(&mut self) {
        let val = self.stack.pop().expect("expected value");
        let local_idx = self.consume_next_byte_as_byte();
        *self.local_var_mut(local_idx) = val;
    }

    /// expects the next byte to be a constants array index to a closure object
    fn handle_closure(&mut self) {
        let constant_idx = unsafe {
            self.ip = self.ip.add(1);
            *self.ip
        };

        let mut closure = match self.get_constant(constant_idx as usize) {
            ConstantValue::Object(o) => match o {
                ConstantObject::Closure(c) => c.clone(),
                _ => panic!("didn't find a closure in constants"),
            },
            _ => panic!("didn't find an object in constants"),
        };

        // upvalues is empty right now, fill it:
        for i in 0..closure.num_upvalues {
            // thought: might be nice to store "num upvalues" as an operand to `Op::Closure` instead of preallocating
            let capture_type: CaptureType = self.consume_next_byte_as_byte().try_into().unwrap();
            let upvalue_index = self.consume_next_byte_as_byte();
            let uv_ptr = match capture_type {
                SurroundingLocal => {
                    let ptr = {
                        let int_start = self.frame().start_idx;
                        let stack_start_ptr = self.stack.as_mut_ptr();
                        let ptr = unsafe {
                            stack_start_ptr.add(int_start as usize + 1 + upvalue_index as usize)
                        };
                        ptr
                    };

                    let uv_ptr = self.capture_upvalue(ptr);
                    uv_ptr
                }
                SurroundingUpvalue => {
                    let uv_ptr = self.frame().closure.upvalues[upvalue_index as usize];
                    uv_ptr
                }
            };
            closure.upvalues[i] = uv_ptr;
            // println!();
        }

        let heap_closure = unsafe { self.allocate_value(ObjectValue::Closure(closure.clone())) };

        self.stack.push(SmallVal::ObjectPtr(heap_closure));
        self.advance();
    }

    // fn capture_upvalue(&mut self, idx_in_stack: usize) -> *mut HeapObject {
    fn capture_upvalue(&mut self, stack_local: *mut SmallVal) -> *mut HeapObject {
        println!("capturing upvalue");
        let mut previous_upvalue: *mut HeapObject = std::ptr::null_mut();
        let mut current = self.open_upvalues;

        // walk the linked list of upvalues
        while !current.is_null() && as_upvalue(current).location > stack_local {
            previous_upvalue = current;
            current = unsafe { &*current }.next;
        }

        if !current.is_null() && as_upvalue(current).location == stack_local {
            // we found an existing upvalue
            return current;
        }

        // Either we reached the end of the list or we went past the location without finding it.
        // In either case, we need to create a new upvalue and insert it into the list.

        let new_upvalue =
            unsafe { self.allocate_value(ObjectValue::UpValue(UpValue::new(stack_local))) };

        if previous_upvalue.is_null() {
            println!("inserting at start");
            // insert at the beginning of the list
            self.open_upvalues = new_upvalue;
        } else {
            println!("inserting in middle");
            // insert in the middle of the list
            as_upvalue(previous_upvalue).next = new_upvalue;
        }

        as_upvalue(new_upvalue).next = current;

        new_upvalue
    }

    // fn handle_quote(&mut self) {
    //     let val = self.stack.pop().unwrap();
    //     let addr = unsafe { self.allocate_value(ObjectValue::SmallValue(val)) };
    //     self.stack.push(SmallVal::Quote(addr));
    //     self.advance();
    // }

    fn handle_return(&mut self) {
        let CallFrame {
            closure,
            return_address,
            start_idx: stack_frame_start,
        } = self
            .callframes
            .pop()
            .expect("expected a call frame to return from");

        self.ip = return_address;

        // clean up the stack
        let return_val = self.stack.pop().expect("expected a return value");

        self.close_upvalues(stack_frame_start as *mut SmallVal);

        // pop the           arguments,       locals, and      function
        self.stack.pop_n(closure.f.arity + closure.f.num_locals + 1);
        if self.stack.ptr + 1 != stack_frame_start {
            panic!(
                "expected stack ptr to be at start of frame ({}), but was at {}",
                stack_frame_start, self.stack.ptr
            );
        }
        // todo use this instead I think:
        // self.stack.ptr = stack_frame_start as usize - 1;

        self.stack.push(return_val);
        self.advance();
    }

    fn close_upvalues(&mut self, last: *mut SmallVal) {
        while !self.open_upvalues.is_null() && as_upvalue(self.open_upvalues).location >= last {
            unsafe {
                // img:  https://craftinginterpreters.com/image/closures/closing.png
                let upvalue = as_upvalue(self.open_upvalues);
                upvalue.closed_val = Some(upvalue.location.read());
                upvalue.location = upvalue.closed_val.as_mut().unwrap();
                self.open_upvalues = upvalue.next;
            }
        }
    }

    fn handle_reference_local(&mut self) {
        let offset = self.consume_next_byte_as_byte();
        let value = self.local_var_mut(offset).clone();
        self.stack.push(value);
        self.advance();
    }

    /// includes function and arguments
    /// [function, arg1, arg2, ... argN, local1, ...]
    fn local_var_mut(&mut self, n: u8) -> &mut SmallVal {
        let global_offset = (self.frame().start_idx + n as i32) as usize;

        self.stack.at_mut(global_offset).unwrap()
    }

    fn handle_func_call(&mut self) {
        // expects the stack to be:
        // [..., function, arg1, arg2, ... argN]
        // and the operand to be the arity of the function, so we can lookup the function and args
        let given_arity = self.consume_next_byte_as_byte() as usize;
        // let callframe = self.frame();

        match self
            .stack
            .peek_back(given_arity /* + callframe.num_locals*/)
            .unwrap()
        {
            SmallVal::ObjectPtr(obj) => match &unsafe { &*obj }.value {
                ObjectValue::Closure(func_obj) => {
                    // ObjectValue::Function(func_obj) => {
                    if func_obj.f.arity != given_arity {
                        self.runtime_error(
                            format!(
                                "arity mismatch: Expected {} arguments, got {}",
                                func_obj.f.arity, given_arity
                            )
                            .as_str(),
                        )
                    }

                    self.callframes.push(self.make_callframe(func_obj.clone()));

                    // set to the start of the function
                    self.ip = func_obj.f.bytecode.code.as_ptr();

                    // allocate space for the locals so they don't get overwritten
                    // args are already at the top of the stack
                    self.stack.ptr += func_obj.f.num_locals as i32;
                }
                ObjectValue::BuiltIn(b) => {
                    let args = self.stack.pop_n(given_arity).unwrap();
                    let result = (b.func)(args, self);
                    self.stack.pop(); // pop off function too
                    self.stack.push(result);
                    self.advance();
                }
                _ => panic!("expected ObjectValue::Function"),
            },
            got => panic!("expected StackValue::Object, got {:?}", got),
        };
    }

    fn make_callframe(&self, closure: Closure) -> CallFrame {
        let arity = closure.f.arity;
        let stack_frame_start = self.stack.ptr - arity as i32;
        // println!(
        //     "at stack_frame_start: {:?}",
        //     match self.stack.at(stack_frame_start as usize).unwrap() {
        //         SmallVal::ObjectPtr(o) => match &unsafe { &**o }.value {
        //             ObjectValue::Closure(c) => &c.f.name,
        //             _ => panic!("expected closure"),
        //         },
        //         _ => panic!("expected object ptr"),
        //     }
        // );
        if stack_frame_start < 0 {
            panic!("stack underflow");
        }
        CallFrame {
            closure,
            return_address: self.ip,
            start_idx: stack_frame_start,
        }
    }

    fn handle_print(&mut self) {
        let val = self.stack.pop().unwrap();
        println!("{}", val);
        self.advance();
    }

    fn handle_reference_global(&mut self) {
        let name = match self.consume_next_byte_as_constant() {
            SmallVal::ObjectPtr(ptr) => match &unsafe { &*ptr }.value {
                ObjectValue::String(s) => s,
                got => panic!(
                    "expected ObjectPtr to be String for reference, got {:?}",
                    got
                ),
            },
            constant_val => panic!(
                "expected constant to be ObjectPtr(String) for reference, got constant {:?}",
                constant_val
            ),
        };
        let global = self.globals.get(name).unwrap_or_else(|| {
            self.runtime_error(format!("undefined global variable: {}", name).as_str());
        });
        self.stack.push(global.clone());
        self.advance();
    }

    fn handle_declare_global(&mut self) {
        let value = self.stack.pop().unwrap();
        let name = self.consume_next_byte_as_constant();
        match name {
            SmallVal::ObjectPtr(ptr) => match &unsafe { &*ptr }.value {
                ObjectValue::String(s) => {
                    self.globals.insert(s.clone(), value);
                }
                _ => panic!("expected string"),
            },
            _ => panic!("expected string"),
        }
        self.advance();
    }

    fn handle_local_define(&mut self) {
        let value = self.stack.pop().unwrap();
        let local_idx = self.consume_next_byte_as_byte();
        *self.local_var_mut(local_idx) = value;
        self.advance();
    }

    fn handle_div(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Integer(a / b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Float(a / b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Float(a as f64 / b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Float(a / b as f64),
            _ => panic!("expected integer or float"),
        });
        self.advance();
    }

    fn handle_mul(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Integer(a * b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Float(a * b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Float(a as f64 * b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Float(a * b as f64),
            _ => panic!("expected integer or float"),
        });
        self.advance();
    }

    fn handle_sub(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Integer(a - b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Float(a - b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Float(a as f64 - b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Float(a - b as f64),
            _ => panic!("expected integer or float"),
        });
        self.advance();
    }

    fn handle_gt(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Bool(a > b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Bool(a > b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Bool(a as f64 > b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Bool(a > b as f64),
            _ => panic!("expected integer or float"),
        });
        self.advance();
    }

    fn handle_lt(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Bool(a < b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Bool(a < b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Bool((a as f64) < b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Bool(a < b as f64),
            _ => panic!("expected integer or float"),
        });
        self.advance();
    }

    fn handle_gte(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Bool(a >= b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Bool(a >= b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Bool(a as f64 >= b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Bool(a >= b as f64),
            _ => panic!("expected integer or float"),
        });
        self.advance();
    }

    fn handle_lte(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        self.stack.push(match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Bool(a <= b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Bool(a <= b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Bool(a as f64 <= b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Bool(a <= b as f64),
            _ => panic!("expected integer or float"),
        });
        self.advance();
    }

    fn handle_add(&mut self) {
        let b = self.stack.pop().unwrap();
        let a = self.stack.pop().unwrap();
        let result = match (a, b) {
            (SmallVal::Integer(a), SmallVal::Integer(b)) => SmallVal::Integer(a + b),
            (SmallVal::Float(a), SmallVal::Float(b)) => SmallVal::Float(a + b),
            (SmallVal::Integer(a), SmallVal::Float(b)) => SmallVal::Float(a as f64 + b),
            (SmallVal::Float(a), SmallVal::Integer(b)) => SmallVal::Float(a + b as f64),
            _ => panic!("expected integer or float"),
        };
        self.stack.push(result);
        self.advance();
    }

    fn handle_jump(&mut self) {
        let offset = self.consume_next_byte_as_byte() as usize;
        self.ip = unsafe { self.ip.add(offset) };
    }

    fn handle_cond_jump(&mut self) {
        let mut offset = self.consume_next_byte_as_byte() as usize;
        let cond_val = self.stack.pop().unwrap();
        if !cond_val.truthy() {
            offset = 1;
        };
        self.ip = unsafe { self.ip.add(offset) };
    }

    fn handle_constant(&mut self) {
        let constant = self.consume_next_byte_as_constant();
        self.stack.push(constant);
        self.advance();
    }

    fn get_constant(&self, idx: usize) -> &ConstantValue {
        if let Some(frame) = &self.callframes.last() {
            return &frame.closure.f.bytecode.constants[idx];
        };
        &self.chunk_constants[idx]
    }

    fn consume_next_byte_as_constant(&mut self) -> SmallVal {
        let constant_idx = self.consume_next_byte_as_byte();
        let constant = self.get_constant(constant_idx as usize).clone();
        self.constant_to_value(constant)
    }

    fn constant_to_value(&mut self, constant: ConstantValue) -> SmallVal {
        match constant {
            ConstantValue::Integer(i) => SmallVal::Integer(i),
            ConstantValue::Float(f) => SmallVal::Float(f),
            ConstantValue::Boolean(b) => SmallVal::Bool(b),
            ConstantValue::Nil => SmallVal::Nil,
            ConstantValue::Object(value) => {
                let obj_ptr = unsafe {
                    self.allocate_value(match value {
                        ConstantObject::String(s) => ObjectValue::String(s),
                        ConstantObject::Symbol(s) => ObjectValue::Symbol(s),
                        ConstantObject::Closure(c) => ObjectValue::Closure(c),
                    })
                };
                #[cfg(feature = "gc_debug")]
                println!("allocated ptr to {}", &unsafe { &*obj_ptr });
                SmallVal::ObjectPtr(obj_ptr)
            }
            ConstantValue::List(list) => self.allocate_list(list),
            ConstantValue::Quote(constant) => {
                let val = self.constant_to_value(*constant);
                let obj_ptr = self.val_to_obj(val);
                SmallVal::Quote(obj_ptr)
            }
        }
    }

    fn allocate_list(&mut self, mut list: Vec<ConstantValue>) -> SmallVal {
        let last = list.pop().unwrap();
        let head = list;

        let last_val = self.constant_to_value(last);
        let last_ptr = self.val_to_obj(last_val);

        let mut cons_cell_ptr = unsafe {
            self.allocate_value(ObjectValue::ConsCell(ConsCell(
                last_ptr,
                std::ptr::null_mut(),
            )))
        };

        for item in head.into_iter().rev() {
            let val = self.constant_to_value(item);
            let item_ptr = unsafe { self.allocate_value(ObjectValue::SmallValue(val)) };

            cons_cell_ptr = unsafe {
                self.allocate_value(ObjectValue::ConsCell(ConsCell(item_ptr, cons_cell_ptr)))
            };
        }

        SmallVal::ObjectPtr(cons_cell_ptr)
    }

    fn val_to_obj(&mut self, val: SmallVal) -> *mut HeapObject {
        match val {
            SmallVal::Integer(_) | SmallVal::Float(_) | SmallVal::Bool(_) | SmallVal::Nil => unsafe {
                self.allocate_value(ObjectValue::SmallValue(val))
            },
            SmallVal::ObjectPtr(ptr) => ptr,
            SmallVal::Quote(ptr) => ptr,
        }
    }

    fn consume_next_byte_as_byte(&mut self) -> u8 {
        unsafe {
            self.advance();
            *self.ip
        }
    }

    fn advance(&mut self) {
        unsafe {
            self.ip = self.ip.add(1);
        }
    }

    pub unsafe fn allocate_value(&mut self, obj_value: ObjectValue) -> *mut HeapObject {
        self.gc();

        // manually allocate the value on the heap
        // let obj_val_ptr = Box::into_raw(Box::new(obj_value.clone())); // todo remove
        // match obj_value {
        //     ObjectValue::SmallValue(sv) => match sv {
        //         SmallVal::Integer(_) => todo!(),
        //         SmallVal::Float(_) => todo!(),
        //         SmallVal::Bool(_) => todo!(),
        //         SmallVal::Nil => todo!(),
        //         SmallVal::ObjectPtr(_) => todo!(),
        //         SmallVal::Quote(_) => todo!(),
        //     },
        //     ObjectValue::String(s) => todo!(),
        //     ObjectValue::Closure(closure) => todo!(),
        //     ObjectValue::Symbol(s) => todo!(),
        //     ObjectValue::ConsCell(ConsCell(a, b)) => todo!(),
        //     ObjectValue::BuiltIn(bi) => todo!(),
        //     ObjectValue::UpValue(uv) => todo!(),
        // }

        // #[cfg(feature = "gc_debug")]
        // println!(
        //     "allocating ptr to {}, bytes: {}",
        //     &*obj_val_ptr,
        //     std::mem::size_of_val(&obj_value)
        // );

        let obj_ptr = alloc(Layout::new::<HeapObject>()) as *mut HeapObject;

        let obj = HeapObject {
            next: self.heap,
            // value: obj_val_ptr,
            value: obj_value,
        };

        obj_ptr.write(obj);

        self.heap = obj_ptr;

        obj_ptr
    }

    fn runtime_error(&self, message: &str) -> ! {
        for frame in self.callframes.iter() {
            println!("in {:?}", frame.closure.f.name);
        }
        panic!("Runtime error: {}", message);
    }
}

fn as_upvalue<'a>(current: *mut HeapObject) -> &'a mut UpValue {
    unsafe {
        let upvalue = &mut (*current).value;
        match upvalue {
            ObjectValue::UpValue(u) => u,
            _ => panic!("expected upvalue"),
        }
    }
}

fn indent(s: String, level: usize) -> String {
    let indent = "  ".repeat(level);
    s.lines()
        .map(|line| format!("{}{}", indent, line))
        .collect::<Vec<String>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load() {
        let mut vm = VM::default();
        let chunk = BytecodeChunk {
            code: vec![Op::Constant.into(), 0x00, Op::DebugEnd.into()],
            constants: vec![ConstantValue::Integer(5)],
        };
        vm.run(chunk);
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack.at(0).unwrap(), &SmallVal::Integer(5));
    }

    #[test]
    fn test_simple_math() {
        let mut vm = VM::default();
        // push 5 push 6 add
        // 5 + 6 = 11
        let chunk = BytecodeChunk {
            code: vec![
                Op::Constant.into(),
                0,
                Op::Constant.into(),
                1,
                Op::Add.into(),
                Op::DebugEnd.into(),
            ],
            constants: vec![ConstantValue::Integer(5), ConstantValue::Integer(6)],
        };
        vm.run(chunk);
        assert_eq!(vm.stack.peek_top().unwrap(), &SmallVal::Integer(11))
    }

    #[test]
    fn test_cond() {
        let bytecode = vec![
            Op::Constant.into(),
            0,
            Op::CondJump.into(),
            5, // jump to the load
            Op::Constant.into(),
            1,
            Op::Jump.into(),
            3, // jump to the end
            Op::Constant.into(),
            2,
            Op::DebugEnd.into(),
        ];
        let ptr = bytecode.as_ptr();

        let mut vm = VM::default();
        vm.run(BytecodeChunk {
            code: bytecode,
            constants: vec![
                ConstantValue::Integer(1),
                ConstantValue::Integer(3),
                ConstantValue::Integer(2),
            ],
        });
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack.at(0).unwrap(), &SmallVal::Integer(2));
        assert_eq!(vm.ip, unsafe { ptr.add(10) }); // idx after the last byte
    }

    #[test]
    fn test_cond_not() {
        let chunk = BytecodeChunk {
            // (if 0 2 3)
            // 0 is truthy
            code: vec![
                Op::Constant.into(),
                0,
                Op::CondJump.into(),
                5,
                Op::Constant.into(),
                1,
                Op::Jump.into(),
                3,
                Op::Constant.into(),
                2,
                Op::DebugEnd.into(),
            ],
            constants: vec![
                ConstantValue::Integer(0),
                ConstantValue::Integer(3),
                ConstantValue::Integer(2),
            ],
        };
        let ptr = chunk.code.as_ptr();

        let mut vm = VM::default();
        vm.run(chunk);
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack.at(0).unwrap(), &SmallVal::Integer(2));
        assert_eq!(vm.ip, unsafe { ptr.add(10) });
    }

    #[test]
    fn test_string() {
        let chunk = BytecodeChunk {
            code: vec![Op::Constant.into(), 0, Op::DebugEnd.into()],
            constants: vec![ConstantValue::Object(ConstantObject::String(
                "Hello, world!".to_string(),
            ))],
        };
        let ptr = chunk.code.as_ptr();

        let mut vm = VM::default();
        vm.run(chunk);
        assert_eq!(vm.stack.len(), 1);

        let string = match vm.stack.peek_top().unwrap() {
            SmallVal::ObjectPtr(ptr) => match &unsafe { &**ptr }.value {
                ObjectValue::String(str) => str,
                _ => panic!(),
            },
            _ => panic!(),
        };

        assert_eq!(string, "Hello, world!");
        assert_eq!(vm.ip, unsafe { ptr.add(2) });
    }

    #[test]
    fn test_function() {
        let bc = BytecodeChunk {
            code: vec![
                Op::Constant.into(),
                0, // load the function
                Op::Constant.into(),
                1, // load the argument 20
                Op::Constant.into(),
                2, // load the argument 30
                Op::FuncCall.into(),
                2, // call the function with 2 arguments
                Op::DebugEnd.into(),
            ],
            constants: vec![
                ConstantValue::Object(ConstantObject::Closure(Closure {
                    f: Function {
                        num_locals: 0,
                        name: "asdf".to_string(),
                        arity: 2,
                        bytecode: Box::new(BytecodeChunk {
                            code: vec![
                                Op::ReferenceLocal.into(),
                                1,
                                Op::ReferenceLocal.into(),
                                2,
                                Op::Add.into(),
                                Op::Return.into(),
                            ],
                            constants: vec![],
                        }),
                    },
                    upvalues: vec![],
                    num_upvalues: 0,
                })),
                ConstantValue::Integer(20),
                ConstantValue::Integer(30),
            ],
        };

        let mut vm = VM::default();
        vm.run(bc);
        assert_eq!(vm.stack.peek_top().unwrap(), &SmallVal::Integer(50));
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack.at(0).unwrap(), &SmallVal::Integer(50));
    }

    #[test]
    fn test_advanced() {
        let bc = BytecodeChunk {
            code: vec![
                Op::Constant.into(),
                0, // load the function
                Op::Constant.into(),
                1, // load the argument 20
                Op::Constant.into(),
                2, // load the argument 30
                Op::FuncCall.into(),
                2, // call the function with 2 arguments
                Op::DebugEnd.into(),
            ],
            constants: vec![
                ConstantValue::Object(ConstantObject::Closure(Closure {
                    f: Function {
                        num_locals: 0,
                        name: "asdf".to_string(),
                        arity: 2,
                        bytecode: Box::new(BytecodeChunk {
                            code: vec![
                                Op::ReferenceLocal.into(),
                                1,
                                Op::ReferenceLocal.into(),
                                2,
                                Op::Add.into(),
                                Op::Return.into(),
                            ],
                            constants: vec![],
                        }),
                    },
                    upvalues: vec![],
                    num_upvalues: 0,
                })),
                ConstantValue::Integer(20),
                ConstantValue::Integer(30),
            ],
        };

        let mut vm = VM::default();
        vm.run(bc);
        assert_eq!(vm.stack.peek_top().unwrap(), &SmallVal::Integer(50));
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack.at(0).unwrap(), &SmallVal::Integer(50));
    }
}
