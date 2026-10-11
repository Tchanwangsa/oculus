//! A minimal JSON reader for the fixtures.

#[derive(Debug)]
pub(super) enum Json {
    Str(String),
    Num(f64),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
    Lit,
}

impl Json {
    pub(super) fn get(&self, key: &str) -> &Json {
        match self {
            Json::Obj(fields) => &fields.iter().find(|(k, _)| k == key).expect(key).1,
            _ => panic!("not an object"),
        }
    }

    pub(super) fn str(&self) -> &str {
        match self {
            Json::Str(s) => s,
            _ => panic!("not a string"),
        }
    }
}

struct Reader<'a> {
    s: &'a [u8],
    i: usize,
}

impl Reader<'_> {
    fn ws(&mut self) {
        while self.i < self.s.len() && self.s[self.i].is_ascii_whitespace() {
            self.i += 1;
        }
    }

    fn value(&mut self) -> Json {
        self.ws();
        match self.s[self.i] {
            b'"' => Json::Str(self.string()),
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b']' {
                        self.i += 1;
                        return Json::Arr(items);
                    }
                    items.push(self.value());
                    self.ws();
                    if self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'{' => {
                self.i += 1;
                let mut fields = Vec::new();
                loop {
                    self.ws();
                    if self.s[self.i] == b'}' {
                        self.i += 1;
                        return Json::Obj(fields);
                    }
                    let key = self.string();
                    self.ws();
                    self.i += 1; // ':'
                    fields.push((key, self.value()));
                    self.ws();
                    if self.s[self.i] == b',' {
                        self.i += 1;
                    }
                }
            }
            b'-' | b'0'..=b'9' => {
                let start = self.i;
                while self.i < self.s.len()
                    && matches!(
                        self.s[self.i],
                        b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                    )
                {
                    self.i += 1;
                }
                Json::Num(
                    std::str::from_utf8(&self.s[start..self.i])
                        .unwrap()
                        .parse()
                        .unwrap(),
                )
            }
            _ => {
                while self.i < self.s.len() && self.s[self.i].is_ascii_alphabetic() {
                    self.i += 1;
                }
                Json::Lit
            }
        }
    }

    fn hex4(&mut self) -> u32 {
        let h = std::str::from_utf8(&self.s[self.i..self.i + 4]).unwrap();
        self.i += 4;
        u32::from_str_radix(h, 16).unwrap()
    }

    fn string(&mut self) -> String {
        self.i += 1;
        let mut out = String::new();
        loop {
            let start = self.i;
            while self.s[self.i] != b'"' && self.s[self.i] != b'\\' {
                self.i += 1;
            }
            out.push_str(std::str::from_utf8(&self.s[start..self.i]).unwrap());
            if self.s[self.i] == b'"' {
                self.i += 1;
                return out;
            }
            self.i += 1;
            let c = self.s[self.i];
            self.i += 1;
            match c {
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'r' => out.push('\r'),
                b'b' => out.push('\u{8}'),
                b'f' => out.push('\u{c}'),
                b'u' => {
                    let mut u = self.hex4();
                    if (0xd800..0xdc00).contains(&u) && self.s[self.i] == b'\\' {
                        self.i += 2;
                        let low = self.hex4();
                        u = 0x10000 + ((u - 0xd800) << 10) + (low - 0xdc00);
                    }
                    out.push(char::from_u32(u).unwrap_or('\u{fffd}'));
                }
                c => out.push(c as char),
            }
        }
    }
}

pub(super) fn read_json(path: &str) -> Json {
    let text = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{path}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    Reader {
        s: text.as_bytes(),
        i: 0,
    }
    .value()
}
