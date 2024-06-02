use std::{collections::HashMap, hash::Hash, marker::PhantomData};

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
enum Type {
    I64,
}

// TODO: see https://docs.rs/ustr/latest/ustr/index.html for a more performant implementation
//       aiming to serialize/deserialize variable length data into memory
pub struct Cache<T> {
    map: HashMap<T, usize>,
    storage: Vec<T>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self {
            map: HashMap::default(),
            storage: Vec::default(),
        }
    }
}

impl<T: Hash + Eq + Clone> Cache<T> {
    pub fn insert<I: Into<T>>(&mut self, val: I) -> CacheIdx<T> {
        let val: T = val.into();
        if let Some(&idx) = self.map.get(&val) {
            return CacheIdx(idx, PhantomData);
        }

        let idx = self.storage.len();
        self.storage.push(val.clone());
        self.map.insert(val, idx);

        CacheIdx(idx, PhantomData)
    }

    pub fn get(&self, idx: CacheIdx<T>) -> &T {
        &self.storage[idx.0]
    }
}
pub struct CacheIdx<T>(usize, PhantomData<T>);

impl<T> Clone for CacheIdx<T> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

impl<T> Copy for CacheIdx<T> {}

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub enum Operand {
    ImmI64(i64),
    Reg(u32),
}

#[derive(Copy, Clone, Debug)]
pub enum Value {
    ImmI64(i64),
}

#[derive(Copy, Clone, Hash, Eq, PartialEq, Debug)]
pub struct Label(u32);

#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub enum Instruction {
    MOV {
        dst: Operand,
        src: Operand,
    }, // move
    ADD {
        dst: Operand,
        src_lhs: Operand,
        src_rhs: Operand,
    }, // addition
    BGE {
        jump: Label,
        lhs: Operand,
        rhs: Operand,
    }, // branch greater than
    BLT {
        jump: Label,
        lhs: Operand,
        rhs: Operand,
    }, // branch less than
    JUMP(Label),
    RET {
        ret: Operand,
    }, // return
}

struct InstructionList {
    idx: CacheIdx<Instruction>,
    next: Option<usize>,
    prev: Option<usize>,
}

struct Register {
    ty: CacheIdx<Type>,
    name: CacheIdx<String>,
}

struct Func {
    identifier: CacheIdx<String>,
    registers: Vec<Register>,
    instructions: Vec<InstructionList>,
}

impl Func {
    fn append(&mut self, insn: CacheIdx<Instruction>) -> usize {
        if self.instructions.is_empty() {
            self.instructions.push(InstructionList {
                idx: insn,
                next: None,
                prev: None,
            });
            return 0;
        }

        let id = self.instructions.len();
        let last = id - 1;
        self.instructions[last].next = Some(self.instructions.len());
        self.instructions.push(InstructionList {
            idx: insn,
            next: None,
            prev: Some(last),
        });
        id
    }
}

fn main() {
    let mut strings = Cache::<String>::default();
    let mut instructions = Cache::<Instruction>::default();
    let mut types = Cache::<Type>::default();
    let mut labels = Vec::default();

    let ty_i64 = types.insert(Type::I64);

    let mut func = Func {
        identifier: strings.insert("loop"),
        registers: vec![
            Register {
                ty: ty_i64,
                name: strings.insert("num"),
            },
            Register {
                ty: ty_i64,
                name: strings.insert("count"),
            },
        ],
        instructions: Vec::default(),
    };

    let num = 0;
    let count = 1;

    let end = Label(0);
    let cond = Label(1);

    // count = 0
    func.append(instructions.insert(Instruction::MOV {
        dst: Operand::Reg(count),
        src: Operand::ImmI64(0),
    }));

    // while count < num { .. }
    let addr_cond = func.append(instructions.insert(Instruction::BGE {
        jump: end,
        lhs: Operand::Reg(count),
        rhs: Operand::Reg(num),
    }));
    // count += 1
    func.append(instructions.insert(Instruction::ADD {
        dst: Operand::Reg(count),
        src_lhs: Operand::Reg(count),
        src_rhs: Operand::ImmI64(1),
    }));
    func.append(instructions.insert(Instruction::JUMP(cond)));

    // return count
    let addr_end = func.append(instructions.insert(Instruction::RET {
        ret: Operand::Reg(count),
    }));

    labels.push(addr_end);
    labels.push(addr_cond);

    // Interpreter
    //
    // opt: resolve the reg/imm parts once instead of on every instruction

    let mut pc = 0;
    let mut stack = vec![Value::ImmI64(0); func.registers.len()];
    stack[num as usize] = Value::ImmI64(5);

    loop {
        let insn_list = &func.instructions[pc];
        let insn = instructions.get(insn_list.idx);

        match insn {
            Instruction::MOV { src, dst } => {
                let val_src = match src {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };

                match dst {
                    Operand::Reg(reg) => stack[*reg as usize] = val_src,
                    _ => panic!("invalid mov dst"),
                };

                pc += 1;
            }
            Instruction::ADD {
                dst,
                src_lhs,
                src_rhs,
            } => {
                let val_lhs = match src_lhs {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };
                let val_rhs = match src_rhs {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };

                let val_add = match (val_lhs, val_rhs) {
                    (Value::ImmI64(lhs), Value::ImmI64(rhs)) => Value::ImmI64(lhs + rhs),
                };

                match dst {
                    Operand::Reg(reg) => stack[*reg as usize] = val_add,
                    _ => panic!("invalid add dst"),
                };

                pc += 1;
            }
            Instruction::BGE { jump, lhs, rhs } => {
                let val_lhs = match lhs {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };
                let val_rhs = match rhs {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };

                match (val_lhs, val_rhs) {
                    (Value::ImmI64(lhs), Value::ImmI64(rhs)) => {
                        if lhs >= rhs {
                            pc = labels[jump.0 as usize];
                        } else {
                            pc += 1;
                        }
                    }
                };
            }
            Instruction::BLT { jump, lhs, rhs } => {
                let val_lhs = match lhs {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };
                let val_rhs = match rhs {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };

                match (val_lhs, val_rhs) {
                    (Value::ImmI64(lhs), Value::ImmI64(rhs)) => {
                        if lhs < rhs {
                            pc = labels[jump.0 as usize];
                        } else {
                            pc += 1;
                        }
                    }
                };
            }
            Instruction::JUMP(jump) => {
                pc = labels[jump.0 as usize];
            }
            Instruction::RET { ret } => {
                let val = match ret {
                    Operand::ImmI64(i) => Value::ImmI64(*i),
                    Operand::Reg(reg) => stack[*reg as usize],
                };

                dbg!(val);

                break;
            }
        }
    }
}
