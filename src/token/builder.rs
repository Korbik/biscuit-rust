use super::{Biscuit, Block};
use crate::crypto::KeyPair;
use crate::datalog::{
    self, SymbolTable, ID,
};
use crate::error;
use rand_core::{CryptoRng, RngCore};
use std::{fmt, convert::{TryInto, TryFrom}, time::{SystemTime, Duration, UNIX_EPOCH}, collections::HashSet};

// reexport those because the builder uses the same definitions
pub use crate::datalog::{IntConstraint, StrConstraint, BytesConstraint};

#[derive(Clone, Debug)]
pub struct BlockBuilder {
    pub index: u32,
    pub facts: Vec<Fact>,
    pub rules: Vec<Rule>,
    pub caveats: Vec<Caveat>,
    pub context: Option<String>,
}

impl BlockBuilder {
    pub fn new(index: u32) -> BlockBuilder {
        BlockBuilder {
            index,
            facts: vec![],
            rules: vec![],
            caveats: vec![],
            context: None,
        }
    }

    pub fn add_fact<F: TryInto<Fact>>(&mut self, fact: F) -> Result<(), error::Token> {
        let fact = fact.try_into().map_err(|_| error::Token::ParseError)?;
        self.facts.push(fact);
        Ok(())
    }

    pub fn add_rule<R: TryInto<Rule>>(&mut self, rule: R) -> Result<(), error::Token> {
        let rule = rule.try_into().map_err(|_| error::Token::ParseError)?;
        self.rules.push(rule);
        Ok(())
    }

    pub fn add_caveat<C: TryInto<Caveat>>(&mut self, caveat: C) -> Result<(), error::Token> {
        let caveat = caveat.try_into().map_err(|_| error::Token::ParseError)?;
        self.caveats.push(caveat);
        Ok(())
    }

    pub fn set_context(&mut self, context: String) {
        self.context = Some(context);
    }

    pub fn build(self, mut symbols: SymbolTable) -> Block {
        let symbols_start = symbols.symbols.len();

        let mut facts = Vec::new();
        for fact in self.facts {
            facts.push(fact.convert(&mut symbols));
        }

        let mut rules = Vec::new();
        for rule in self.rules {
            rules.push(rule.convert(&mut symbols));
        }

        let mut caveats = Vec::new();
        for caveat in self.caveats {
            caveats.push(caveat.convert(&mut symbols));
        }
        let new_syms = SymbolTable {
            symbols: symbols.symbols.split_off(symbols_start),
        };

        Block {
            index: self.index,
            symbols: new_syms,
            facts,
            rules,
            caveats,
            context: self.context,
        }
    }

    pub fn check_right(&mut self, right: &str) {
        let caveat = rule(
            "check_right",
            &[s(right)],
            &[
                pred("resource", &[s("ambient"), var("resource_name")]),
                pred("operation", &[s("ambient"), s(right)]),
                pred("right", &[s("authority"), var("resource_name"), s(right)]),
            ],
        );

        let _ = self.add_caveat(caveat);
    }

    pub fn check_resource(&mut self, resource: &str) {
        let caveat = rule(
            "resource_check",
            &[s("resource_check")],
            &[pred("resource", &[s("ambient"), string(resource)])],
        );

        let _ = self.add_caveat(caveat);
    }

    pub fn check_operation(&mut self, operation: &str) {
        let caveat = rule(
            "operation_check",
            &[s("operation_check")],
            &[pred("operation", &[s("ambient"), s(operation)])],
        );

        let _ = self.add_caveat(caveat);
    }

    pub fn resource_prefix(&mut self, prefix: &str) {
        let caveat = constrained_rule(
            "prefix",
            &[var("resource")],
            &[pred("resource", &[s("ambient"), var("resource")])],
            &[Constraint {
                id: "resource".to_string(),
                kind: ConstraintKind::String(datalog::StrConstraint::Prefix(prefix.to_string())),
            }],
        );

        let _ = self.add_caveat(caveat);
    }

    pub fn resource_suffix(&mut self, suffix: &str) {
        let caveat = constrained_rule(
            "suffix",
            &[var("resource")],
            &[pred("resource", &[s("ambient"), var("resource")])],
            &[Constraint {
                id: "resource".to_string(),
                kind: ConstraintKind::String(datalog::StrConstraint::Suffix(suffix.to_string())),
            }],
        );

        let _ = self.add_caveat(caveat);
    }

    pub fn expiration_date(&mut self, date: SystemTime) {
        let caveat = constrained_rule(
            "expiration",
            &[var("date")],
            &[pred("time", &[s("ambient"), var("date")])],
            &[Constraint {
                id: "date".to_string(),
                kind: ConstraintKind::Date(DateConstraint::Before(date)),
            }],
        );

        let _ = self.add_caveat(caveat);
    }

    pub fn revocation_id(&mut self, id: i64) {
        let _ = self.add_fact(fact("revocation_id", &[int(id)]));
    }
}

#[derive(Clone)]
pub struct BiscuitBuilder<'a> {
    root: &'a KeyPair,
    pub symbols_start: usize,
    pub symbols: SymbolTable,
    pub facts: Vec<datalog::Fact>,
    pub rules: Vec<datalog::Rule>,
    pub caveats: Vec<datalog::Caveat>,
    pub context: Option<String>,
}

impl<'a> BiscuitBuilder<'a> {
    pub fn new(
        root: &'a KeyPair,
        base_symbols: SymbolTable,
    ) -> BiscuitBuilder<'a> {
        BiscuitBuilder {
            root,
            symbols_start: base_symbols.symbols.len(),
            symbols: base_symbols,
            facts: vec![],
            rules: vec![],
            caveats: vec![],
            context: None,
        }
    }

    pub fn add_authority_fact<F: TryInto<Fact>>(&mut self, fact: F) -> Result<(), error::Token> {
        let fact = fact.try_into().map_err(|_| error::Token::ParseError)?;

        let f = fact.convert(&mut self.symbols);
        self.facts.push(f);
        Ok(())
    }

    pub fn add_authority_rule<Ru: TryInto<Rule>>(&mut self, rule: Ru) -> Result<(), error::Token> {
        let rule = rule.try_into().map_err(|_| error::Token::ParseError)?;

        let r = rule.convert(&mut self.symbols);
        self.rules.push(r);
        Ok(())
    }

    pub fn add_authority_caveat<Ru: TryInto<Rule>>(&mut self, rule: Ru) -> Result<(), error::Token> {
        let caveat = rule.try_into().map_err(|_| error::Token::ParseError)?;
        let r = caveat.convert(&mut self.symbols);
        self.caveats.push(datalog::Caveat { queries: vec![r]});
        Ok(())
    }

    pub fn add_right(&mut self, resource: &str, right: &str) {
        let _ = self.add_authority_fact(fact(
            "right",
            &[s("authority"), string(resource), s(right)],
        ));
    }

    pub fn set_context(&mut self, context: String) {
        self.context = Some(context);
    }

    pub fn build<R: RngCore + CryptoRng>(&self, rng: &'a mut R) -> Result<Biscuit, error::Token> {
        let new_syms = SymbolTable { symbols: self.symbols.clone().symbols.split_off(self.symbols_start) };

        let authority_block = Block {
            index: 0,
            symbols: new_syms,
            facts: self.facts.clone(),
            rules: self.rules.clone(),
            caveats: self.caveats.clone(),
            context: self.context.clone(),
        };

        Biscuit::new(rng, self.root, self.symbols.clone(), authority_block)
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Atom {
    Symbol(String),
    Variable(String),
    Integer(i64),
    Str(String),
    Date(u64),
    Bytes(Vec<u8>),
}

impl Atom {
    pub fn convert(&self, symbols: &mut SymbolTable) -> ID {
        match self {
            Atom::Symbol(s) => ID::Symbol(symbols.insert(s)),
            Atom::Variable(s) => ID::Variable(symbols.insert(s) as u32),
            Atom::Integer(i) => ID::Integer(*i),
            Atom::Str(s) => ID::Str(s.clone()),
            Atom::Date(d) => ID::Date(*d),
            Atom::Bytes(s) => ID::Bytes(s.clone()),
        }
    }

    pub fn convert_from(f: &datalog::ID, symbols: &SymbolTable) -> Self {
      match f {
        ID::Symbol(s) => Atom::Symbol(symbols.print_symbol(*s)),
        ID::Variable(s) => Atom::Variable(symbols.print_symbol(*s as u64)),
        ID::Integer(i) => Atom::Integer(*i),
        ID::Str(s) => Atom::Str(s.clone()),
        ID::Date(d) => Atom::Date(*d),
        ID::Bytes(s) => Atom::Bytes(s.clone()),
      }
    }
}

impl From<&Atom> for Atom {
    fn from(i: &Atom) -> Self {
        match i {
            Atom::Symbol(ref s) => Atom::Symbol(s.clone()),
            Atom::Variable(ref v) => Atom::Variable(v.clone()),
            Atom::Integer(ref i) => Atom::Integer(*i),
            Atom::Str(ref s) => Atom::Str(s.clone()),
            Atom::Date(ref d) => Atom::Date(*d),
            Atom::Bytes(ref s) => Atom::Bytes(s.clone()),
        }
    }
}

impl AsRef<Atom> for Atom {
    fn as_ref(&self) -> &Atom {
        self
    }
}

impl fmt::Display for Atom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Atom::Variable(i) => write!(f, "${}", i),
            Atom::Integer(i) => write!(f, "{}", i),
            Atom::Str(s) => write!(f, "\"{}\"", s),
            Atom::Symbol(s) => write!(f, "#{}", s),
            Atom::Date(d) => {
                let t = UNIX_EPOCH + Duration::from_secs(*d);
                write!(f, "{:?}", t)
            }
            Atom::Bytes(s) => write!(f, "hex:{}", hex::encode(s)),
        }

    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct Predicate {
    pub name: String,
    pub ids: Vec<Atom>,
}

impl Predicate {
    pub fn convert(&self, symbols: &mut SymbolTable) -> datalog::Predicate {
        let name = symbols.insert(&self.name);
        let mut ids = vec![];

        for id in self.ids.iter() {
            ids.push(id.convert(symbols));
        }

        datalog::Predicate { name, ids }
    }

    pub fn convert_from(p: &datalog::Predicate, symbols: &SymbolTable) -> Self {
        Predicate {
          name: symbols.print_symbol(p.name),
          ids: p.ids.iter().map(|id| Atom::convert_from(&id, symbols)).collect(),
        }
    }

    pub fn new(name: String, ids: &[Atom]) -> Predicate {
        Predicate {
            name,
            ids: ids.to_vec(),
        }
    }
}

impl AsRef<Predicate> for Predicate {
    fn as_ref(&self) -> &Predicate {
        self
    }
}

impl fmt::Display for Predicate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.name)?;

        if self.ids.len() > 0 {
            write!(f, "{}", self.ids[0])?;

            if self.ids.len() > 1 {
                for i in 1..self.ids.len() {
                    write!(f, ", {}", self.ids[i])?;
                }
            }
        }
        write!(f, ")")
    }
}


#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct Fact(pub Predicate);

impl Fact {
    pub fn new(name: String, ids: &[Atom]) -> Fact {
        Fact(Predicate::new(name, ids))
    }
}

impl Fact {
    pub fn convert(&self, symbols: &mut SymbolTable) -> datalog::Fact {
        datalog::Fact {
            predicate: self.0.convert(symbols),
        }
    }

    pub fn convert_from(f: &datalog::Fact, symbols: &SymbolTable) -> Self {
        Fact(Predicate::convert_from(&f.predicate, symbols))
    }
}

impl fmt::Display for Fact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Constraint {
    pub id: String,
    pub kind: ConstraintKind,
}

impl Constraint {
    pub fn convert(&self, symbols: &mut SymbolTable) -> datalog::Constraint {
        datalog::Constraint {
          // this conversion should be fine, the symbol table will not grow to
          // more than u32::MAX entries
          id: symbols.insert(&self.id) as u32,
          kind: self.kind.convert(symbols),
        }
    }

    pub fn convert_from(c: &datalog::Constraint, symbols: &SymbolTable) -> Self {
        Constraint {
            id: symbols.print_symbol(c.id as u64),
            kind: ConstraintKind::convert_from(&c.kind, symbols),
        }
    }
}

impl AsRef<Constraint> for Constraint {
    fn as_ref(&self) -> &Constraint {
        self
    }
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            ConstraintKind::Integer(IntConstraint::Lower(i)) => write!(f, "${} < {}", self.id, i),
            ConstraintKind::Integer(IntConstraint::Larger(i)) => write!(f, "${} > {}", self.id, i),
            ConstraintKind::Integer(IntConstraint::LowerOrEqual(i)) => write!(f, "${} <= {}", self.id, i),
            ConstraintKind::Integer(IntConstraint::LargerOrEqual(i)) => write!(f, "${} >= {}", self.id, i),
            ConstraintKind::Integer(IntConstraint::Equal(i)) => write!(f, "${} == {}", self.id, i),
            ConstraintKind::Integer(IntConstraint::In(i)) => write!(f, "${} in {:?}", self.id, i),
            ConstraintKind::Integer(IntConstraint::NotIn(i)) => write!(f, "${} not in {:?}", self.id, i),
            ConstraintKind::String(StrConstraint::Prefix(i)) => write!(f, "${} matches {}*", self.id, i),
            ConstraintKind::String(StrConstraint::Suffix(i)) => write!(f, "${} matches *{}", self.id, i),
            ConstraintKind::String(StrConstraint::Equal(i)) => write!(f, "${} == {}", self.id, i),
            ConstraintKind::String(StrConstraint::Regex(i)) => write!(f, "${} matches /{}/", self.id, i),
            ConstraintKind::String(StrConstraint::In(i)) => write!(f, "${} in {:?}", self.id, i),
            ConstraintKind::String(StrConstraint::NotIn(i)) => write!(f, "${} not in {:?}", self.id, i),
            ConstraintKind::Date(DateConstraint::Before(date)) => {
              //let date = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(*i as i64, 0), Utc);
              let date: chrono::DateTime<chrono::Utc> = (*date).into();
              write!(f, "${} <= {}", self.id, date.to_rfc3339())
            },
            ConstraintKind::Date(DateConstraint::After(date)) => {
              //let date = DateTime::<Utc>::from_utc(NaiveDateTime::from_timestamp(*i as i64, 0), Utc);
              let date: chrono::DateTime<chrono::Utc> = (*date).into();
              write!(f, "${} >= {}", self.id, date.to_rfc3339())
            },
            ConstraintKind::Symbol(SymbolConstraint::In(i)) => write!(f, "${} in {:?}", self.id, i),
            ConstraintKind::Symbol(SymbolConstraint::NotIn(i)) => {
                write!(f, "${} not in {:?}", self.id, i)
            },
            ConstraintKind::Bytes(BytesConstraint::Equal(i)) => write!(f, "${} == {}", self.id, hex::encode(i)),
            ConstraintKind::Bytes(BytesConstraint::In(i)) => {
                write!(f, "${} in {:?}", self.id, i.iter()
                       .map(|s| format!("hex:{}", hex::encode(s))).collect::<HashSet<_>>())
            },
            ConstraintKind::Bytes(BytesConstraint::NotIn(i)) => {
                write!(f, "${} not in {:?}", self.id, i.iter()
                       .map(|s| format!("hex:{}", hex::encode(s))).collect::<HashSet<_>>())
            },
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub enum ConstraintKind {
    Integer(datalog::IntConstraint),
    String(datalog::StrConstraint),
    Date(DateConstraint),
    Symbol(SymbolConstraint),
    Bytes(datalog::BytesConstraint),
}

impl ConstraintKind {
    pub fn convert(&self, symbols: &mut SymbolTable) -> datalog::ConstraintKind {
      match self {
        ConstraintKind::Integer(i) => datalog::ConstraintKind::Int(i.clone()),
        ConstraintKind::String(s) => datalog::ConstraintKind::Str(s.clone()),
        ConstraintKind::Bytes(s) => datalog::ConstraintKind::Bytes(s.clone()),
        ConstraintKind::Date(DateConstraint::Before(date)) => {
          let dur = date.duration_since(UNIX_EPOCH).expect("date should be after Unix Epoch");
          datalog::ConstraintKind::Date(datalog::DateConstraint::Before(dur.as_secs()))
        },
        ConstraintKind::Date(DateConstraint::After(date)) => {
          let dur = date.duration_since(UNIX_EPOCH).expect("date should be after Unix Epoch");
          datalog::ConstraintKind::Date(datalog::DateConstraint::After(dur.as_secs()))
        }
        ConstraintKind::Symbol(SymbolConstraint::In(h)) => {
          let hset = h.iter().map(|s| symbols.insert(&s)).collect();
          datalog::ConstraintKind::Symbol(datalog::SymbolConstraint::In(hset))
        },
        ConstraintKind::Symbol(SymbolConstraint::NotIn(h)) => {
          let hset = h.iter().map(|s| symbols.insert(&s)).collect();
          datalog::ConstraintKind::Symbol(datalog::SymbolConstraint::NotIn(hset))
        },
      }
    }

    pub fn convert_from(c: &datalog::ConstraintKind, symbols: &SymbolTable) -> Self {
      match c {
        datalog::ConstraintKind::Int(i) => ConstraintKind::Integer(i.clone()),
        datalog::ConstraintKind::Str(s) => ConstraintKind::String(s.clone()),
        datalog::ConstraintKind::Date(datalog::DateConstraint::Before(secs)) => {
          let date = UNIX_EPOCH + Duration::from_secs(*secs);
          ConstraintKind::Date(DateConstraint::Before(date))
        },
        datalog::ConstraintKind::Date(datalog::DateConstraint::After(secs)) => {
          let date = UNIX_EPOCH + Duration::from_secs(*secs);
          ConstraintKind::Date(DateConstraint::After(date))
        }
        datalog::ConstraintKind::Symbol(datalog::SymbolConstraint::In(h)) => {
          let hset = h.iter().map(|s| symbols.print_symbol(*s)).collect();
          ConstraintKind::Symbol(SymbolConstraint::In(hset))
        },
        datalog::ConstraintKind::Symbol(datalog::SymbolConstraint::NotIn(h)) => {
          let hset = h.iter().map(|s| symbols.print_symbol(*s)).collect();
          ConstraintKind::Symbol(SymbolConstraint::NotIn(hset))
        },
        datalog::ConstraintKind::Bytes(s) => ConstraintKind::Bytes(s.clone()),
      }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DateConstraint {
    Before(SystemTime),
    After(SystemTime),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolConstraint {
    In(HashSet<String>),
    NotIn(HashSet<String>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rule(
    pub Predicate,
    pub Vec<Predicate>,
    pub Vec<Constraint>,
);

impl Rule {
    pub fn convert(&self, symbols: &mut SymbolTable) -> datalog::Rule {
        let head = self.0.convert(symbols);
        let mut body = vec![];
        let mut constraints = vec![];

        for p in self.1.iter() {
            body.push(p.convert(symbols));
        }

        for c in self.2.iter() {
            constraints.push(c.convert(symbols));
        }

        datalog::Rule {
            head,
            body,
            constraints,
        }
    }

    pub fn convert_from(r: &datalog::Rule, symbols: &SymbolTable) -> Self {
        Rule(
            Predicate::convert_from(&r.head, symbols),
            r.body.iter().map(|p| Predicate::convert_from(p, symbols)).collect(),
            r.constraints.iter().map(|c| Constraint::convert_from(c, symbols)).collect(),
        )
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "*{} <- ", self.0)?;

        if self.1.len() > 0 {
            write!(f, "{}", self.1[0])?;

            if self.1.len() > 1 {
                for i in 1..self.1.len() {
                    write!(f, ", {}", self.1[i])?;
                }
            }
        }

        if self.2.len() > 0 {
            write!(f, " @ {}", self.2[0])?;

            if self.2.len() > 1 {
                for i in 1..self.2.len() {
                    write!(f, ", {}", self.2[i])?;
                }
            }

        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Caveat {
    pub queries: Vec<Rule>,
}

impl Caveat {
    pub fn convert(&self, symbols: &mut SymbolTable) -> datalog::Caveat {
        let mut queries = vec![];
        for q in self.queries.iter() {
            queries.push(q.convert(symbols));
        }

        datalog::Caveat { queries }
    }
}

impl TryFrom<Rule> for Caveat {
    type Error = error::Token;

    fn try_from(value: Rule) -> Result<Self, Self::Error> {
        Ok(Caveat { queries: vec![value] })
    }
}

impl TryFrom<&[Rule]> for Caveat {
    type Error = error::Token;

    fn try_from(values: &[Rule]) -> Result<Self, Self::Error> {
        Ok(Caveat { queries: values.to_vec() })
    }
}

impl fmt::Display for Caveat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.queries.len() > 0 {
            write!(f, "{}", self.queries[0])?;

            if self.queries.len() > 1 {
                for i in 1..self.queries.len() {
                    write!(f, " || {}", self.queries[i])?;
                }
            }
        }

        Ok(())
    }
}

/// creates a new fact
pub fn fact<I: AsRef<Atom>>(name: &str, ids: &[I]) -> Fact {
    Fact(pred(name, ids))
}

/// creates a predicate
pub fn pred<I: AsRef<Atom>>(name: &str, ids: &[I]) -> Predicate {
    Predicate {
        name: name.to_string(),
        ids: ids.iter().map(|id| id.as_ref().clone()).collect(),
    }
}

/// creates a rule
pub fn rule<I: AsRef<Atom>, P: AsRef<Predicate>>(
    head_name: &str,
    head_ids: &[I],
    predicates: &[P],
) -> Rule {
    Rule(
        pred(head_name, head_ids),
        predicates.iter().map(|p| p.as_ref().clone()).collect(),
        Vec::new(),
    )
}

/// creates a rule with constraints
pub fn constrained_rule<I: AsRef<Atom>, P: AsRef<Predicate>, C: AsRef<Constraint>>(
    head_name: &str,
    head_ids: &[I],
    predicates: &[P],
    constraints: &[C],
) -> Rule {
    Rule(
        pred(head_name, head_ids),
        predicates.iter().map(|p| p.as_ref().clone()).collect(),
        constraints.iter().map(|c| c.as_ref().clone()).collect(),
    )
}

/// creates an integer value
pub fn int(i: i64) -> Atom {
    Atom::Integer(i)
}

/// creates a string
pub fn string(s: &str) -> Atom {
    Atom::Str(s.to_string())
}

/// creates a symbol
///
/// once the block is generated, this symbol will be added to the symbol table if needed
pub fn s(s: &str) -> Atom {
    Atom::Symbol(s.to_string())
}

/// creates a symbol
///
/// once the block is generated, this symbol will be added to the symbol table if needed
pub fn symbol(s: &str) -> Atom {
    Atom::Symbol(s.to_string())
}

/// creates a date
///
/// internally the date will be stored as seconds since UNIX_EPOCH
pub fn date(t: &SystemTime) -> Atom {
    let dur = t.duration_since(UNIX_EPOCH).unwrap();
    Atom::Date(dur.as_secs())
}

/// creates a variable for a rule
pub fn var(s: &str) -> Atom {
    Atom::Variable(s.to_string())
}

/// creates a variable for a rule
pub fn variable(s: &str) -> Atom {
    Atom::Variable(s.to_string())
}

/// creates a byte array
pub fn bytes(s: &[u8]) -> Atom {
    Atom::Bytes(s.to_vec())
}
