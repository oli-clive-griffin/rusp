#![allow(unused, dead_code)]

use crate::sexpr::Sexpr;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::{
    alloc::{alloc, dealloc, Layout},
    collections::HashMap,
    hash::Hash, panic::PanicInfo,
};

/// THOUGHTS
/// First thought is that we may be able to mirror evaluator.~.eval with "produce bytecode that "

/// simple stack-based virtual machine for integer arithmetic
pub struct VM {
    ip: *const u8,
    // code: Vec<u8>,
    // function_table: Vec<ByteCodeFunction>,
    pub stack: Vec<StackValue>, // todo remove pub
    pub globals: HashMap<String, StackValue>,
    callframes: Vec<CallFrame>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ObjectValue {
    String(String),
    Function(Box<BytecodeChunk>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeapObject {
    next: *mut HeapObject,
    value: ObjectValue,
    // marked: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StackValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Nil,
    Object(*mut HeapObject),
}

#[derive(Debug, Clone, PartialEq)]
struct CallFrame {
    return_address: *const u8,
    stack_frame_start: *const StackValue,
    arity: usize
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConstantsValue {
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Nil,
    Object(ObjectValue),
}

impl StackValue {
    fn truthy(&self) -> bool {
        match self {
            StackValue::Integer(i) => *i != 0,
            StackValue::Float(f) => *f != 0.0,
            StackValue::Boolean(b) => *b,
            StackValue::Nil => false,
            StackValue::Object(_) => false,
            // StackValue::Addr(addr) => panic!("should not happen, (calling truthy on address {:?})", addr),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BytecodeChunk {
    pub code: Vec<u8>,
    pub constants: Vec<ConstantsValue>,
}
impl BytecodeChunk {
    pub fn new(code: Vec<u8>, constants: Vec<ConstantsValue>) -> Self {
        BytecodeChunk { code, constants }
    }
}

// struct ByteCodeFunction {
//     _name: String,
//     arity: usize,
//     bytecode: Vec<u8>,
// }

#[repr(u8)]
#[derive(Debug, PartialEq, Clone, IntoPrimitive, TryFromPrimitive)]
pub enum Op {
    Constant = 0,
    Local = 1,
    Add = 2,
    Sub = 3,
    Mul = 4,
    Div = 5,
    Neg = 6,
    Jump = 7,     // jumps to the specified address
    CondJump = 8, // jumps to the specified address if the top of the stack is not zero
    FuncCall = 9,
    Return = 10,
    DeclareGlobal = 11,
    Reference = 12,
    DebugEnd = 254,   // ends the program
    DebugPrint = 255, // prints the stack
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    pub fn new() -> VM {
        VM {
            ip: std::ptr::null_mut(),
            stack: Vec::default(),
            globals: HashMap::default(),
            callframes: Vec::default(),
        }
    }

    // pub fn load(&mut self, code: Vec<u8>) {
    //     self.code = code;
    // }

    // pub fn run(&mut self) {
    pub fn run(&mut self, chunk: BytecodeChunk) {
        self.ip = chunk.code.as_ptr();
        // let mut end_ptr = unsafe { self.ip.add(chunk.code.len()) };

        loop {
            println!("stack: {:?}", self.stack);
            // probably should switch back to raw bytes
            // but this is nice for development.
            let byte = unsafe { *self.ip }.try_into().unwrap();

            match byte {
                Op::Constant => {
                    let constant = self.consume_next_byte_as_constant(&chunk); // advances here
                    self.stack.push(constant);
                    self.advance();
                }
                Op::CondJump => {
                    let mut offset = self.consume_next_byte_as_byte() as usize;
                    let cond_val = self.stack.pop().unwrap();

                    if !cond_val.truthy() {
                        offset = 1;
                    };

                    self.ip = unsafe { self.ip.add(offset) };
                }
                Op::Jump => {
                    let offset = self.consume_next_byte_as_byte() as usize;
                    self.ip = unsafe { self.ip.add(offset) };
                }
                Op::Add => {
                    // reverse order because we pop from the stack
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(match (a, b) {
                        (StackValue::Integer(a), StackValue::Integer(b)) => {
                            StackValue::Integer(a + b)
                        }
                        (StackValue::Object(a), StackValue::Object(b)) => {
                            match (&unsafe { &*a }.value, &unsafe { &*b }.value) {
                                (ObjectValue::String(a), ObjectValue::String(b)) => {
                                    let obj_ptr = unsafe {
                                        allocate_value(ObjectValue::String(a.clone() + b))
                                    };
                                    StackValue::Object(obj_ptr)
                                }
                                _ => todo!(),
                            }
                        }
                        otherwise => {
                            print!("{:?}", otherwise);
                            unimplemented!()
                        }
                    });
                    self.advance();
                }
                Op::Sub => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(match (a, b) {
                        (StackValue::Integer(a), StackValue::Integer(b)) => {
                            StackValue::Integer(a - b)
                        }
                        _ => todo!(),
                    });
                    self.advance();
                }
                Op::Mul => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(match (a, b) {
                        (StackValue::Integer(a), StackValue::Integer(b)) => {
                            StackValue::Integer(a * b)
                        }
                        _ => todo!(),
                    });
                    self.advance();
                }
                Op::Div => {
                    let b = self.stack.pop().unwrap();
                    let a = self.stack.pop().unwrap();
                    self.stack.push(match (a, b) {
                        (StackValue::Integer(a), StackValue::Integer(b)) => {
                            StackValue::Integer(a / b)
                        }
                        _ => todo!(),
                    });
                    self.advance();
                }
                Op::Neg => {
                    let a = self.stack.pop().unwrap();
                    self.stack.push(match a {
                        StackValue::Integer(a) => StackValue::Integer(-a),
                        _ => todo!(),
                    });
                    self.advance();
                }

                // Op::FuncCall => todo!(),
                // Op::FuncCall => {
                //     // expects the stack to be (top-down):
                //     // [<func_idx>, ...func_args,
                //     let func_idx = self.stack.pop().unwrap();
                //     let ByteCodeFunction {
                //         _name: _,
                //         bytecode,
                //         arity,
                //     } = &self.function_table[func_idx as usize];
                // }
                Op::DeclareGlobal => {
                    let value = self.stack.pop().unwrap();
                    let name = self.consume_next_byte_as_constant(&chunk);
                    match name {
                        StackValue::Object(ptr) => match &unsafe { &*ptr }.value {
                            ObjectValue::String(s) => {
                                self.globals.insert(s.clone(), value);
                            }
                            _ => panic!("expected string"),
                        },
                        _ => panic!("expected string"),
                    }
                    self.advance();
                }
                Op::Reference => {
                    let name = match self.consume_next_byte_as_constant(&chunk) {
                        StackValue::Object(ptr) => match &unsafe { &*ptr }.value {
                            ObjectValue::String(s) => s,
                            _ => panic!("expected string value for reference"),
                        },
                        _ => panic!("expected string value for reference"),
                    };

                    let stack_val = self.globals.get(name).unwrap().clone();
                    self.stack.push(stack_val);
                    self.advance();
                }
                Op::DebugPrint => {
                    let val = match self.stack.pop().unwrap() {
                        StackValue::Object(ptr) => match &unsafe { &*ptr }.value {
                            ObjectValue::String(s) => s.clone(),
                            ObjectValue::Function(f) => "function".to_string(),
                        },
                        StackValue::Integer(i) => i.to_string(),
                        StackValue::Float(f) => f.to_string(),
                        StackValue::Boolean(b) => b.to_string(),
                        StackValue::Nil => "nil".to_string(),
                        // StackValue::Addr(addr) => format!("address: <{:?}>", addr),
                    };

                    println!("{:?}", val);
                    self.advance();
                }
                Op::FuncCall => {
                    // expects the stack to be:
                    // [..., function, arg1, arg2, ... argN]
                    // and the operand to be the arity of the function, so we can lookup the function and args

                    let arity = match self.consume_next_byte_as_constant(&chunk) {
                        StackValue::Integer(arity) => arity,
                        _ => panic!("expected function"),
                    };

                    if arity < 0 {
                        panic!("negative arity");
                    };
                    let func_obj = match self.peek(arity as usize) {
                        StackValue::Object(obj) => match &unsafe { &**obj }.value {
                            ObjectValue::Function(func) => func,
                            _ => panic!("expected function"),
                        },
                        _ => panic!("expected function"),
                    };
                    self.callframes.push(CallFrame {
                        return_address: self.ip,
                        stack_frame_start: unsafe { self.stack.as_ptr().add(self.stack.len() - arity as usize - 1) },
                        arity: arity as usize
                    });
                    self.ip = func_obj.code.as_ptr();
                }
                Op::Local => {
                    let current_callframe = self
                        .callframes
                        .last()
                        .expect("expected a call frame for a local variable");

                    let stack_frame_start = current_callframe.stack_frame_start;

                    let offset = self.consume_next_byte_as_byte() as usize;
                    let value = unsafe { stack_frame_start.add(offset).read() }; // todo understand why ".read()" is needed
                    println!("loading local variable {:?}", value);
                    self.stack.push(value.clone());
                    self.advance()
                }
                Op::Return => {
                    let frame = self.callframes.pop().expect("expected a call frame to return from");
                    self.ip = frame.return_address;

                    // clean up the stack
                    let returnval = self.stack.pop().expect("expected a return value");

                    // pop the arguments
                    for _ in 0..frame.arity as usize {
                        self.stack.pop();
                    }

                    // pop the function
                    self.stack.pop();
                    
                    self.stack.push(returnval);

                    self.advance();
                }
                Op::DebugEnd => {
                    println!("end of program");
                    return;
                }
            }
        }
    }

    fn peek(&self, back: usize) -> &StackValue {
        println!("peeking {:?} back into vec {:?}", back, self.stack);
        &self.stack[self.stack.len() - back - 1]
    }

    fn consume_next_byte_as_constant(&mut self, chunk: &BytecodeChunk) -> StackValue {
        unsafe {
            self.ip = self.ip.add(1); // IMPORTANT: clone
            let constant_idx = *self.ip as usize;
            match chunk.constants[constant_idx].clone() {
                ConstantsValue::Integer(v) => StackValue::Integer(v),
                ConstantsValue::Float(v) => StackValue::Float(v),
                ConstantsValue::Boolean(v) => StackValue::Boolean(v),
                ConstantsValue::Nil => StackValue::Nil,
                ConstantsValue::Object(value) => {
                    let obj_ptr = allocate_value(value);
                    StackValue::Object(obj_ptr)
                }
            }
        }
    }

    fn consume_next_byte_as_byte(&mut self) -> u8 {
        unsafe {
            self.ip = self.ip.add(1);
            *self.ip
        }
    }

    fn advance(&mut self) {
        unsafe {
            self.ip = self.ip.add(1);
        }
    }
}

unsafe fn allocate<T>(obj: T) -> *mut T {
    let obj_ptr = alloc(Layout::new::<T>()) as *mut T;
    obj_ptr.write(obj);
    obj_ptr
}

static mut HEAD: *mut HeapObject = std::ptr::null_mut();
unsafe fn allocate_value(obj_value: ObjectValue) -> *mut HeapObject {
    let obj_ptr = alloc(Layout::new::<HeapObject>()) as *mut HeapObject;
    obj_ptr.write(HeapObject {
        next: HEAD,
        value: obj_value,
    });
    HEAD = obj_ptr;

    // #[cfg(debug_assertions)]
    // print_stack(HEAD);

    obj_ptr
}

// fn print_heap(head_: *mut HeapObject) {
//     unsafe {
//         println!(
//             "allocated {:?} (knowingly leaking memory for now)",
//             (*head_).clone()
//         );
//         println!("heap:");
//         let mut current = head_;
//         while !current.is_null() {
//             println!("- {:?}", &(*current).value);
//             current = (*current).next;
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load() {
        let mut vm = VM::default();
        let chunk = BytecodeChunk {
            code: vec![Op::Constant.into(), 0x00, Op::DebugEnd.into()],
            constants: vec![ConstantsValue::Integer(5)],
        };
        vm.run(chunk);
        assert_eq!(vm.stack, vec![StackValue::Integer(5)])
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
            constants: vec![ConstantsValue::Integer(5), ConstantsValue::Integer(6)],
        };
        vm.run(chunk);
        assert_eq!(vm.stack[0], StackValue::Integer(11))
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
                ConstantsValue::Integer(1),
                ConstantsValue::Integer(3),
                ConstantsValue::Integer(2),
            ],
        });
        assert_eq!(vm.stack, vec![StackValue::Integer(2)]);
        assert_eq!(vm.ip, unsafe { ptr.add(10) }); // idx after the last byte
    }

    #[test]
    fn test_cond_not() {
        let chunk = BytecodeChunk {
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
                ConstantsValue::Integer(0),
                ConstantsValue::Integer(3),
                ConstantsValue::Integer(2),
            ],
        };
        let ptr = chunk.code.as_ptr();

        let mut vm = VM::default();
        vm.run(chunk);
        assert_eq!(vm.stack, vec![StackValue::Integer(3)]);
        assert_eq!(vm.ip, unsafe { ptr.add(10) });
    }

    #[test]
    fn test_string() {
        let chunk = BytecodeChunk {
            code: vec![Op::Constant.into(), 0, Op::DebugEnd.into()],
            constants: vec![ConstantsValue::Object(ObjectValue::String(
                "Hello, world!".to_string(),
            ))],
        };
        let ptr = chunk.code.as_ptr();

        let mut vm = VM::default();
        vm.run(chunk);
        assert_eq!(vm.stack.len(), 1);

        let string = match vm.stack[0] {
            StackValue::Object(ptr) => match &unsafe { &*ptr }.value {
                ObjectValue::String(str) => str,
                _ => panic!(),
            },
            _ => panic!(),
        };

        assert_eq!(string, "Hello, world!");
        assert_eq!(vm.ip, unsafe { ptr.add(2) });
    }

    #[test]
    fn test_string_concat() {
        let chunk = BytecodeChunk {
            code: vec![
                Op::Constant.into(),
                0,
                Op::Constant.into(),
                1,
                Op::Add.into(),
                Op::DebugEnd.into(),
            ],
            constants: vec![
                ConstantsValue::Object(ObjectValue::String("foo".to_string())),
                ConstantsValue::Object(ObjectValue::String("bar".to_string())),
            ],
        };
        let ptr = chunk.code.as_ptr();

        let mut vm = VM::default();
        vm.run(chunk);
        assert_eq!(vm.stack.len(), 1);

        let string = match vm.stack[0] {
            StackValue::Object(ptr) => match &unsafe { &*ptr }.value {
                ObjectValue::String(str) => str,
                _ => panic!(),
            },
            _ => panic!(),
        };

        assert_eq!(string, "foobar");
        assert_eq!(vm.ip, unsafe { ptr.add(5) });
    }

    #[test]
    fn test_var_declare() {
        let chunk = BytecodeChunk {
            code: vec![
                Op::Constant.into(),
                0,
                Op::DeclareGlobal.into(),
                1,
                Op::DebugEnd.into(),
            ],
            constants: vec![
                ConstantsValue::Integer(5),                                     // value
                ConstantsValue::Object(ObjectValue::String("foo".to_string())), // name
            ],
        };
        let ptr = chunk.code.as_ptr();

        let mut vm = VM::default();
        vm.run(chunk);
        assert_eq!(vm.stack.len(), 0);
        assert_eq!(vm.globals.len(), 1);
        assert_eq!(vm.globals.get("foo").unwrap(), &StackValue::Integer(5));
        assert_eq!(vm.ip, unsafe { ptr.add(4) });
    }

    #[test]
    fn test_var_reference() {
        let chunk = BytecodeChunk {
            code: vec![
                Op::Constant.into(),
                0,
                Op::DeclareGlobal.into(),
                1,
                Op::Reference.into(),
                1,
                Op::DebugEnd.into(),
            ],
            constants: vec![
                ConstantsValue::Integer(5),                                     // value
                ConstantsValue::Object(ObjectValue::String("foo".to_string())), // name
            ],
        };
        let ptr = chunk.code.as_ptr();

        let mut vm = VM::default();
        vm.run(chunk);
        assert_eq!(vm.stack.len(), 1);
        assert_eq!(vm.stack[0], StackValue::Integer(5));
        assert_eq!(vm.ip, unsafe { ptr.add(6) });
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
                3, // call the function with 2 arguments
                Op::DebugEnd.into(),
            ],
            constants: vec![
                ConstantsValue::Object(ObjectValue::Function(Box::new(BytecodeChunk {
                    code: vec![
                        Op::Local.into(),
                        // make variables 1-indexed as the function itself is at 0 (maybe? (bad idea? (probably)))
                        1, // load the first argument from back in the stack
                        Op::Local.into(),
                        2, // load the second argument from back in the stack
                        Op::Add.into(),
                        Op::Return.into(),
                    ],
                    constants: vec![],
                }))),
                ConstantsValue::Integer(20),
                ConstantsValue::Integer(30),
                ConstantsValue::Integer(2),
            ],
        };

        let mut vm = VM::default();
        vm.run(bc);
        assert_eq!(vm.stack.last().unwrap(), &StackValue::Integer(50));
        assert_eq!(vm.stack, vec![StackValue::Integer(50)]);
    }
}

// let name = match self.consume_next_byte_as_constant(&chunk) {
//     StackValue::Object(ptr) => match &unsafe { &*ptr }.value {
//         ObjectValue::String(s) => s,
//         _ => panic!("expected string value for reference"),
//     },
//     _ => panic!("expected string value for reference"),
// };

// let name = get_obejct(self, &chunk, String);
// not needed for now
// // #[repr(C)] // for the struct definition
// impl StackValue {
//     fn from_bytes(bytes: [u8; 16]) -> Self {
//         unsafe { mem::transmute(bytes) }
//     }
//     fn to_bytes(self) -> [u8; 16] {
//         unsafe { mem::transmute(self) }
//     }
// }
