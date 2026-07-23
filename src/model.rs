use std::{borrow::ToOwned, str::FromStr};

use crate::date::Date;

#[derive(Debug, Clone)]
pub struct Model {
    pub directives: Vec<Directive>,
}

impl std::fmt::Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for d in &self.directives {
            d.fmt(f)?;
            f.write_str("\n")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Directive {
    pub date: Date,
    pub inner: DirectiveInner,
    pub metadata: Vec<MetadataEntry>,
}

impl std::fmt::Display for Directive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.date.fmt(f)?;
        f.write_str(" ")?;
        self.inner.fmt(f)?;
        for m in &self.metadata {
            m.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MetadataEntry {
    pub key: String,
    pub value: String,
}

impl std::fmt::Display for MetadataEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("\n  {}: {:?}", self.key, self.value))
    }
}

#[derive(Debug, Clone)]
pub enum DirectiveInner {
    Transaction(Transaction),
    Open(Open),
    Close(Close),
    Commodity(Commodity),
    Balance(Balance),
    Pad(Pad),
    Note(Note),
    Document(Document),
    Price(Price),
    Event(Event),
    Query(Query),
    Custom(Custom),
}

impl std::fmt::Display for DirectiveInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transaction(transaction) => transaction.fmt(f),
            Self::Open(open) => open.fmt(f),
            Self::Close(close) => close.fmt(f),
            Self::Commodity(commodity) => commodity.fmt(f),
            Self::Balance(balance) => balance.fmt(f),
            Self::Pad(pad) => pad.fmt(f),
            Self::Note(note) => note.fmt(f),
            Self::Document(document) => document.fmt(f),
            Self::Price(price) => price.fmt(f),
            Self::Event(event) => event.fmt(f),
            Self::Query(query) => query.fmt(f),
            Self::Custom(custom) => custom.fmt(f),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Open {
    pub account: Account,
    pub currencies: Vec<Currency>,
}

impl std::fmt::Display for Open {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("open ")?;
        self.account.fmt(f)?;
        let mut first = true;
        for c in &self.currencies {
            if first {
                f.write_str(" ")?;
                first = false;
            } else {
                f.write_str(",")?;
            }
            c.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Close {
    pub account: Account,
}

impl std::fmt::Display for Close {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("close ")?;
        self.account.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Commodity {
    pub currency: Currency,
}

impl std::fmt::Display for Commodity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("close ")?;
        self.currency.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Balance {
    pub account: Account,
    pub amount: Amount,
}

impl std::fmt::Display for Balance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("balance ")?;
        self.account.fmt(f)?;
        f.write_str(" ")?;
        self.amount.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Pad {
    pub account: Account,
    pub account_pad: Account,
}

impl std::fmt::Display for Pad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("pad ")?;
        self.account.fmt(f)?;
        f.write_str(" ")?;
        self.account_pad.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Document {
    pub account: Account,
    pub path: String,
}

impl std::fmt::Display for Document {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("document {} {:?}", self.account, self.path))
    }
}

#[derive(Debug, Clone)]
pub struct Price {
    pub currency: Currency,
    pub amount: Amount,
}

impl std::fmt::Display for Price {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("price ")?;
        self.currency.fmt(f)?;
        f.write_str(" ")?;
        self.amount.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Event {
    pub name: String,
    pub value: String,
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("event ")?;
        self.name.fmt(f)?;
        f.write_str(" ")?;
        self.value.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Query {
    pub name: String,
    pub contents: String,
}

impl std::fmt::Display for Query {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("query ")?;
        self.name.fmt(f)?;
        f.write_str(" ")?;
        self.contents.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransactionFlag {
    Asterisk,
    Exclamation,
    Txn,
}

impl std::fmt::Display for TransactionFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Asterisk => f.write_str("*"),
            Self::Exclamation => f.write_str("!"),
            Self::Txn => f.write_str("txn"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub flag: TransactionFlag,
    pub narration: String,
    pub second_narration: Option<String>,
    pub rows: Vec<Posting>,
}

impl std::fmt::Display for Transaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{} {:?}", self.flag, self.narration))?;
        if let Some(second_narration) = &self.second_narration {
            f.write_fmt(format_args!(" {second_narration:?}"))?;
        }
        for r in &self.rows {
            f.write_str("\n  ")?;
            r.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct Posting {
    pub flag: Option<TransactionRowFlag>,
    pub account: Account,
    pub amount: Option<Amount>,
    pub rate: Option<String>,
}

impl std::fmt::Display for Posting {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(flag) = &self.flag {
            flag.fmt(f)?;
            f.write_str(" ")?;
        }
        self.account.fmt(f)?;
        if let Some(amount) = &self.amount {
            f.write_str(" ")?;
            amount.fmt(f)?;
        }
        if let Some(rate) = &self.rate {
            f.write_str(" @ ")?;
            rate.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AccountPrefix {
    Assets,
    Liabilities,
    Equity,
    Income,
    Expenses,
}

impl FromStr for AccountPrefix {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Assets" => Ok(Self::Assets),
            "Liabilities" => Ok(Self::Liabilities),
            "Equity" => Ok(Self::Equity),
            "Income" => Ok(Self::Income),
            "Expenses" => Ok(Self::Expenses),
            _ => Err(()),
        }
    }
}

impl std::fmt::Display for AccountPrefix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Assets => f.write_str("Assets"),
            Self::Liabilities => f.write_str("Liabilities"),
            Self::Equity => f.write_str("Equity"),
            Self::Income => f.write_str("Income"),
            Self::Expenses => f.write_str("Expenses"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Account {
    pub prefix: AccountPrefix,
    pub components: Vec<String>,
}

impl FromStr for Account {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut split = s.split(':');
        let prefix = split.next().ok_or(())?.parse()?;
        let components = split.map(ToOwned::to_owned).collect();
        Ok(Self { prefix, components })
    }
}

impl std::fmt::Display for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.prefix.fmt(f)?;
        for component in &self.components {
            f.write_str(":")?;
            component.fmt(f)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct TransactionRowFlag;

impl std::fmt::Display for TransactionRowFlag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("!")
    }
}

#[derive(Debug, Clone)]
pub struct Amount {
    pub value: rust_decimal::Decimal,
    pub unit: Currency,
}

impl std::fmt::Display for Amount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.value.fmt(f)?;
        f.write_str(" ")?;
        self.unit.fmt(f)
    }
}

#[derive(Debug, Clone)]
pub struct Currency(pub String);

impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Note(pub String);

impl std::fmt::Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("note ")?;
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Custom(pub String);

impl std::fmt::Display for Custom {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("custom ")?;
        f.write_str(&self.0)
    }
}
