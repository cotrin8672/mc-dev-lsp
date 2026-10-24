#![allow(dead_code)]
#[derive(Default)]
pub struct ClassFile {
    pool: Vec<Vec<u8>>,
    slots: u16,
    methods: Vec<Method>,
    fields: Vec<(u16, u16, u16)>,
    bootstraps: Vec<(u16, Vec<u16>)>,
}
pub struct Method {
    pub name: u16,
    pub descriptor: u16,
    pub flags: u16,
    pub code: Vec<u8>,
    pub max_locals: u16,
    pub max_stack: u16,
    pub handlers: Vec<(u16, u16, u16, u16)>,
    pub locals: Vec<(u16, u16, u16, u16, u16)>,
    pub stack_map: Vec<u8>,
}
impl ClassFile {
    pub fn item(&mut self, bytes: Vec<u8>, wide: bool) -> u16 {
        let index = self.slots + 1;
        self.slots += if wide { 2 } else { 1 };
        self.pool.push(bytes);
        index
    }
    pub fn utf16(&mut self, value: &[u16]) -> u16 {
        let mut encoded = Vec::new();
        for &unit in value {
            if unit != 0 && unit < 0x80 {
                encoded.push(unit as u8);
            } else if unit < 0x800 {
                encoded.extend([(0xc0 | (unit >> 6)) as u8, (0x80 | (unit & 63)) as u8]);
            } else {
                encoded.extend([
                    (0xe0 | (unit >> 12)) as u8,
                    (0x80 | ((unit >> 6) & 63)) as u8,
                    (0x80 | (unit & 63)) as u8,
                ]);
            }
        }
        let mut bytes = vec![1];
        bytes.extend((encoded.len() as u16).to_be_bytes());
        bytes.extend(encoded);
        self.item(bytes, false)
    }
    pub fn utf(&mut self, value: &str) -> u16 {
        self.utf16(&value.encode_utf16().collect::<Vec<_>>())
    }
    pub fn class(&mut self, name: &str) -> u16 {
        let name = self.utf(name);
        self.item([vec![7], name.to_be_bytes().to_vec()].concat(), false)
    }
    pub fn string(&mut self, value: &str) -> u16 {
        let utf = self.utf(value);
        self.item([vec![8], utf.to_be_bytes().to_vec()].concat(), false)
    }
    pub fn int(&mut self, value: i32) -> u16 {
        self.item([vec![3], value.to_be_bytes().to_vec()].concat(), false)
    }
    pub fn method_type(&mut self, descriptor: &str) -> u16 {
        let utf = self.utf(descriptor);
        self.item([vec![16], utf.to_be_bytes().to_vec()].concat(), false)
    }
    fn name_type(&mut self, name: &str, descriptor: &str) -> u16 {
        let name = self.utf(name);
        let descriptor = self.utf(descriptor);
        self.item(
            [
                vec![12],
                name.to_be_bytes().to_vec(),
                descriptor.to_be_bytes().to_vec(),
            ]
            .concat(),
            false,
        )
    }
    pub fn member(&mut self, tag: u8, owner: &str, name: &str, descriptor: &str) -> u16 {
        let owner = self.class(owner);
        let nt = self.name_type(name, descriptor);
        self.item(
            [
                vec![tag],
                owner.to_be_bytes().to_vec(),
                nt.to_be_bytes().to_vec(),
            ]
            .concat(),
            false,
        )
    }
    pub fn handle(&mut self, kind: u8, member: u16) -> u16 {
        self.item(
            [vec![15, kind], member.to_be_bytes().to_vec()].concat(),
            false,
        )
    }
    pub fn dynamic(
        &mut self,
        name: &str,
        descriptor: &str,
        handle: u16,
        arguments: Vec<u16>,
    ) -> u16 {
        let bootstrap = self.bootstraps.len() as u16;
        self.bootstraps.push((handle, arguments));
        let nt = self.name_type(name, descriptor);
        self.item(
            [
                vec![18],
                bootstrap.to_be_bytes().to_vec(),
                nt.to_be_bytes().to_vec(),
            ]
            .concat(),
            false,
        )
    }
    pub fn field(&mut self, name: &str, descriptor: &str, flags: u16) {
        let name = self.utf(name);
        let descriptor = self.utf(descriptor);
        self.fields.push((flags, name, descriptor));
    }
    pub fn method(
        &mut self,
        name: &str,
        descriptor: &str,
        flags: u16,
        max_locals: u16,
        code: Vec<u8>,
    ) -> &mut Method {
        let name = self.utf(name);
        let descriptor = self.utf(descriptor);
        self.methods.push(Method {
            name,
            descriptor,
            flags,
            max_locals,
            max_stack: 32,
            code,
            handlers: Vec::new(),
            locals: Vec::new(),
            stack_map: Vec::new(),
        });
        self.methods.last_mut().unwrap()
    }
    pub fn finish(self, name: &str) -> Vec<u8> {
        self.finish_with_hierarchy(name, "java/lang/Object", &[], 0x21)
    }
    pub fn finish_with_hierarchy(
        mut self,
        name: &str,
        superclass: &str,
        interfaces: &[&str],
        flags: u16,
    ) -> Vec<u8> {
        let owner = self.class(name);
        let superclass = self.class(superclass);
        let interfaces = interfaces
            .iter()
            .map(|name| self.class(name))
            .collect::<Vec<_>>();
        let code_name = self.utf("Code");
        let locals_name = self.utf("LocalVariableTable");
        let frames_name = self.utf("StackMapTable");
        let bootstrap_name = self.utf("BootstrapMethods");
        let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe, 0, 0, 0, 52];
        bytes.extend((self.slots + 1).to_be_bytes());
        for item in self.pool {
            bytes.extend(item);
        }
        bytes.extend(flags.to_be_bytes());
        bytes.extend(owner.to_be_bytes());
        bytes.extend(superclass.to_be_bytes());
        bytes.extend((interfaces.len() as u16).to_be_bytes());
        for interface in interfaces {
            bytes.extend(interface.to_be_bytes());
        }
        bytes.extend((self.fields.len() as u16).to_be_bytes());
        for (flags, name, descriptor) in self.fields {
            bytes.extend(flags.to_be_bytes());
            bytes.extend(name.to_be_bytes());
            bytes.extend(descriptor.to_be_bytes());
            bytes.extend(0u16.to_be_bytes());
        }
        bytes.extend((self.methods.len() as u16).to_be_bytes());
        for method in self.methods {
            bytes.extend(method.flags.to_be_bytes());
            bytes.extend(method.name.to_be_bytes());
            bytes.extend(method.descriptor.to_be_bytes());
            bytes.extend(1u16.to_be_bytes());
            bytes.extend(code_name.to_be_bytes());
            let mut content = Vec::new();
            content.extend(method.max_stack.to_be_bytes());
            content.extend(method.max_locals.to_be_bytes());
            content.extend((method.code.len() as u32).to_be_bytes());
            content.extend(method.code);
            content.extend((method.handlers.len() as u16).to_be_bytes());
            for (start, end, target, catch) in method.handlers {
                for value in [start, end, target, catch] {
                    content.extend(value.to_be_bytes());
                }
            }
            content.extend(
                (u16::from(!method.locals.is_empty()) + u16::from(!method.stack_map.is_empty()))
                    .to_be_bytes(),
            );
            if !method.locals.is_empty() {
                content.extend(locals_name.to_be_bytes());
                content.extend((2 + method.locals.len() as u32 * 10).to_be_bytes());
                content.extend((method.locals.len() as u16).to_be_bytes());
                for (start, length, name, descriptor, slot) in method.locals {
                    for value in [start, length, name, descriptor, slot] {
                        content.extend(value.to_be_bytes());
                    }
                }
            }
            if !method.stack_map.is_empty() {
                content.extend(frames_name.to_be_bytes());
                content.extend((method.stack_map.len() as u32).to_be_bytes());
                content.extend(method.stack_map);
            }
            bytes.extend((content.len() as u32).to_be_bytes());
            bytes.extend(content);
        }
        bytes.extend(u16::from(!self.bootstraps.is_empty()).to_be_bytes());
        if !self.bootstraps.is_empty() {
            let mut content = Vec::new();
            content.extend((self.bootstraps.len() as u16).to_be_bytes());
            for (handle, args) in self.bootstraps {
                content.extend(handle.to_be_bytes());
                content.extend((args.len() as u16).to_be_bytes());
                for arg in args {
                    content.extend(arg.to_be_bytes());
                }
            }
            bytes.extend(bootstrap_name.to_be_bytes());
            bytes.extend((content.len() as u32).to_be_bytes());
            bytes.extend(content);
        }
        bytes
    }
}
pub fn op(opcode: u8, index: u16) -> Vec<u8> {
    vec![opcode, (index >> 8) as u8, index as u8]
}
pub fn sample() -> Vec<u8> {
    let mut class = ClassFile::default();
    let field = class.member(9, "Sample", "sampleField", "I");
    let trim = class.member(10, "java/lang/String", "trim", "()Ljava/lang/String;");
    let string = class.class("java/lang/String");
    let init = class.member(10, "java/lang/String", "<init>", "()V");
    let hello = class.string("hello");
    class.method(
        "readSampleField",
        "()I",
        1,
        1,
        [vec![0x2a], op(0xb4, field), vec![0xac]].concat(),
    );
    class.method(
        "writeSampleField",
        "(I)V",
        1,
        2,
        [vec![0x2a, 0x1b], op(0xb5, field), vec![0xb1]].concat(),
    );
    class.method("add", "(II)I", 1, 3, vec![0x1b, 0x1c, 0x60, 0xac]);
    class.method(
        "arrayAccess",
        "([II)V",
        1,
        4,
        vec![0x2b, 0x1c, 0x2e, 0x3e, 0xb1],
    );
    class.method(
        "arrayStore",
        "([III)V",
        1,
        4,
        vec![0x2b, 0x1c, 0x1d, 0x4f, 0xb1],
    );
    class.method(
        "assignString",
        "()V",
        1,
        2,
        [op(0x13, hello), vec![0x4c, 0xb1]].concat(),
    );
    class.method(
        "addAndMultiply",
        "(III)I",
        1,
        5,
        vec![0x1b, 0x1c, 0x60, 0x36, 4, 0x15, 4, 0x1d, 0x68, 0xac],
    );
    class.method(
        "intEqualsZero",
        "(I)Z",
        1,
        2,
        vec![0x1b, 0x9a, 0, 5, 0x04, 0xac, 0x03, 0xac],
    );
    class.method(
        "intComparison",
        "(II)Z",
        1,
        3,
        vec![0x1b, 0x1c, 0xa0, 0, 5, 0x04, 0xac, 0x03, 0xac],
    );
    class.method(
        "trim",
        "(Ljava/lang/String;)Ljava/lang/String;",
        1,
        2,
        [vec![0x2b], op(0xb6, trim), vec![0xb0]].concat(),
    );
    class.method(
        "newString",
        "()Ljava/lang/String;",
        1,
        1,
        [op(0xbb, string), vec![0x59], op(0xb7, init), vec![0xb0]].concat(),
    );
    class.method(
        "castString",
        "(Ljava/lang/Object;)Ljava/lang/String;",
        1,
        2,
        [vec![0x2b], op(0xc0, string), vec![0xb0]].concat(),
    );
    class.method(
        "isString",
        "(Ljava/lang/Object;)Z",
        1,
        2,
        [vec![0x2b], op(0xc1, string), vec![0xac]].concat(),
    );
    class.finish("Sample")
}
