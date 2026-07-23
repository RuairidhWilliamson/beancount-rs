#![allow(clippy::unwrap_used)]

use std::convert::Into;

use nom::{
    IResult, Parser as _,
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::digit1,
    combinator::{all_consuming, map, map_res, opt, recognize, value, verify},
    error::context,
    multi::many0,
    sequence::{preceded, terminated},
};

use crate::{
    date::Date,
    model::{AccountPrefix, TransactionFlag},
};

pub fn sort_directive_runs(mut statements: &mut [Statement]) {
    let Some(start) = statements.iter().position(Statement::is_directive) else {
        return;
    };
    statements = &mut statements[start..];
    let Some(end) = statements.iter().position(|s| !Statement::is_directive(s)) else {
        sort_directives(statements);
        return;
    };
    let (before, rest) = statements.split_at_mut(end);
    sort_directives(before);
    // IDEA: Could use tail recursion
    sort_directive_runs(rest);
}

fn sort_directives(statements: &mut [Statement]) {
    statements.sort_by(|a, b| {
        let Statement::Directive(a) = a else {
            unreachable!()
        };
        let Statement::Directive(b) = b else {
            unreachable!()
        };
        a.date
            .cmp(&b.date)
            .then(a.is_transaction().cmp(&b.is_transaction()))
    });
}

pub fn statements(input: &str) -> IResult<&str, Vec<Statement<'_>>> {
    all_consuming(many0(statement)).parse(input)
}

fn statement(input: &str) -> IResult<&str, Statement<'_>> {
    let newline = map(tag("\n"), |_| Statement::Newline);
    let comment = map(
        preceded(tag(";"), terminated(is_not("\n"), opt(tag("\n")))),
        Statement::Comment,
    );
    let option = map(
        preceded(tag("option "), terminated(is_not("\n"), opt(tag("\n")))),
        Statement::Option,
    );
    let include = map(
        preceded(tag("include "), terminated(is_not("\n"), opt(tag("\n")))),
        Statement::Include,
    );
    let plugin = map(
        preceded(tag("plugin "), terminated(is_not("\n"), opt(tag("\n")))),
        Statement::Plugin,
    );
    let pushtag = map(
        preceded(tag("pushtag "), terminated(is_not("\n"), opt(tag("\n")))),
        Statement::PushTag,
    );
    let poptag = map(
        preceded(tag("poptag "), terminated(is_not("\n"), opt(tag("\n")))),
        Statement::PopTag,
    );
    let directive = map(terminated(directive, opt(tag("\n"))), Statement::Directive);

    let mut parser = alt((
        newline, comment, option, include, plugin, pushtag, poptag, directive,
    ));
    parser.parse(input)
}

#[derive(Debug, Clone)]
pub enum Statement<'src> {
    Newline,
    Comment(&'src str),
    Option(&'src str),
    Include(&'src str),
    Plugin(&'src str),
    PushTag(&'src str),
    PopTag(&'src str),
    Directive(Directive<'src>),
}

impl Statement<'_> {
    pub fn is_newline(&self) -> bool {
        matches!(self, Self::Newline)
    }

    pub fn is_directive(&self) -> bool {
        matches!(self, Self::Directive(_))
    }

    pub fn is_transaction(&self) -> bool {
        match self {
            Self::Directive(directive) => directive.is_transaction(),
            _ => false,
        }
    }
}

impl std::fmt::Display for Statement<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Newline => f.write_str("\n"),
            Self::Comment(comment) => {
                f.write_str(";")?;
                f.write_str(comment)?;
                f.write_str("\n")
            }
            Self::Option(option) => {
                f.write_str("option ")?;
                f.write_str(option)?;
                f.write_str("\n")
            }
            Self::Include(include) => {
                f.write_str("include ")?;
                f.write_str(include)?;
                f.write_str("\n")
            }
            Self::Plugin(plugin) => {
                f.write_str("plugin ")?;
                f.write_str(plugin)?;
                f.write_str("\n")
            }
            Self::PushTag(tag) => {
                f.write_str("pushtag ")?;
                f.write_str(tag)
            }
            Self::PopTag(tag) => {
                f.write_str("poptag ")?;
                f.write_str(tag)
            }
            Self::Directive(directive) => {
                directive.fmt(f)?;
                f.write_str("\n")
            }
        }
    }
}

fn date(input: &str) -> IResult<&str, Date> {
    let year = map_res(digit1, str::parse);
    let month = context(
        "month must be between 1 and 12",
        verify(map_res(digit1, str::parse), |m| (1..=12).contains(m)),
    );
    let day = context(
        "day must be between 1 and 31",
        verify(map_res(digit1, str::parse), |d| (1..=31).contains(d)),
    );
    map_res(
        (terminated(year, tag("-")), month, preceded(tag("-"), day)),
        |(year, month, day)| Date::new(year, month, day),
    )
    .parse(input)
}

#[derive(Debug, Clone)]
pub struct Directive<'src> {
    date: Date,
    inner: DirectiveInner<'src>,
    metadata: Vec<MetadataEntry<'src>>,
}

impl TryFrom<Directive<'_>> for crate::model::Directive {
    type Error = rust_decimal::Error;

    fn try_from(value: Directive<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            date: value.date,
            inner: value.inner.try_into()?,
            metadata: value.metadata.into_iter().map(Into::into).collect(),
        })
    }
}

impl Directive<'_> {
    fn is_transaction(&self) -> bool {
        matches!(self.inner, DirectiveInner::Transaction(_))
    }
}

impl std::fmt::Display for Directive<'_> {
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

fn directive(input: &str) -> IResult<&str, Directive<'_>> {
    context(
        "directive",
        map(
            (
                terminated(date, tag(" ")),
                directive_inner,
                many0(metadata_entry),
            ),
            |(date, inner, metadata)| Directive {
                date,
                inner,
                metadata,
            },
        ),
    )
    .parse(input)
}

#[derive(Debug, Clone)]
struct MetadataEntry<'src> {
    key: &'src str,
    value: DoubleQuotedString<'src>,
}

impl std::fmt::Display for MetadataEntry<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\n  ")?;
        self.key.fmt(f)?;
        f.write_str(": ")?;
        self.value.fmt(f)
    }
}

impl From<MetadataEntry<'_>> for crate::model::MetadataEntry {
    fn from(value: MetadataEntry<'_>) -> Self {
        Self {
            key: value.key.into(),
            value: value.value.to_value(),
        }
    }
}

fn metadata_entry(input: &str) -> IResult<&str, MetadataEntry<'_>> {
    context(
        "metadata_entry",
        map(
            (
                preceded(tag("\n  "), is_not(":")),
                preceded(tag(": "), double_quoted_string),
            ),
            |(key, value)| MetadataEntry { key, value },
        ),
    )
    .parse(input)
}

#[derive(Debug, Clone)]
enum DirectiveInner<'src> {
    Transaction(Transaction<'src>),
    Open(Open<'src>),
    Close(Account<'src>),
    Commodity(&'src str),
    Balance(Balance<'src>),
    Pad(Pad<'src>),
    Note(&'src str),
    Document(Document<'src>),
    Price(Price<'src>),
    Event(&'src str),
    Query(&'src str),
    Custom(&'src str),
}

impl TryFrom<DirectiveInner<'_>> for crate::model::DirectiveInner {
    type Error = rust_decimal::Error;

    fn try_from(value: DirectiveInner) -> Result<Self, Self::Error> {
        Ok(match value {
            DirectiveInner::Transaction(transaction) => Self::Transaction(transaction.try_into()?),
            DirectiveInner::Open(open) => Self::Open(open.into()),
            DirectiveInner::Close(account) => Self::Close(crate::model::Close {
                account: account.into(),
            }),
            DirectiveInner::Commodity(commodity) => Self::Commodity(crate::model::Commodity {
                currency: crate::model::Currency(commodity.into()),
            }),
            DirectiveInner::Balance(balance) => Self::Balance(crate::model::Balance {
                account: balance.account.into(),
                amount: balance.amount.try_into()?,
            }),
            DirectiveInner::Pad(pad) => Self::Pad(pad.into()),
            DirectiveInner::Note(s) => Self::Note(crate::model::Note(s.into())),
            DirectiveInner::Document(document) => Self::Document(crate::model::Document {
                account: document.account.into(),
                path: document.path.to_value(),
            }),
            DirectiveInner::Price(price) => Self::Price(crate::model::Price {
                currency: crate::model::Currency(price.currency.into()),
                amount: price.amount.try_into()?,
            }),
            DirectiveInner::Event(_) => todo!(),
            DirectiveInner::Query(_) => todo!(),
            DirectiveInner::Custom(s) => Self::Custom(crate::model::Custom(s.into())),
        })
    }
}

impl std::fmt::Display for DirectiveInner<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DirectiveInner::Transaction(transaction) => transaction.fmt(f),
            DirectiveInner::Open(open) => {
                f.write_str("open ")?;
                open.fmt(f)
            }
            DirectiveInner::Close(close) => {
                f.write_str("close ")?;
                close.fmt(f)
            }
            DirectiveInner::Commodity(commodity) => {
                f.write_str("commodity ")?;
                f.write_str(commodity)
            }
            DirectiveInner::Balance(balance) => {
                f.write_str("balance ")?;
                balance.fmt(f)
            }
            DirectiveInner::Pad(pad) => {
                f.write_str("pad ")?;
                pad.fmt(f)
            }
            DirectiveInner::Note(note) => {
                f.write_str("note ")?;
                f.write_str(note)
            }
            DirectiveInner::Document(document) => {
                f.write_str("document ")?;
                document.fmt(f)
            }
            DirectiveInner::Price(price) => {
                f.write_str("price ")?;
                price.fmt(f)
            }
            DirectiveInner::Event(event) => {
                f.write_str("event ")?;
                f.write_str(event)
            }
            DirectiveInner::Query(query) => {
                f.write_str("query ")?;
                f.write_str(query)
            }
            DirectiveInner::Custom(custom) => {
                f.write_str("custom ")?;
                f.write_str(custom)
            }
        }
    }
}

fn directive_inner(input: &str) -> IResult<&str, DirectiveInner<'_>> {
    alt((
        map(preceded(tag("open "), Open::parse), DirectiveInner::Open),
        map(
            preceded(tag("close "), Account::parse),
            DirectiveInner::Close,
        ),
        map(
            preceded(tag("commodity "), is_not("\n")),
            DirectiveInner::Commodity,
        ),
        map(
            preceded(tag("balance "), Balance::parse),
            DirectiveInner::Balance,
        ),
        map(preceded(tag("pad "), Pad::parse), DirectiveInner::Pad),
        map(preceded(tag("note "), is_not("\n")), DirectiveInner::Note),
        map(
            preceded(tag("document "), Document::parse),
            DirectiveInner::Document,
        ),
        map(preceded(tag("price "), Price::parse), DirectiveInner::Price),
        map(preceded(tag("event "), is_not("\n")), DirectiveInner::Event),
        map(preceded(tag("query "), is_not("\n")), DirectiveInner::Query),
        map(
            preceded(tag("custom "), is_not("\n")),
            DirectiveInner::Custom,
        ),
        map(transaction, DirectiveInner::Transaction),
    ))
    .parse(input)
}

#[derive(Debug, Clone)]
struct Balance<'src> {
    account: Account<'src>,
    amount: Amount<'src>,
}

impl TryFrom<Balance<'_>> for crate::model::Balance {
    type Error = rust_decimal::Error;

    fn try_from(value: Balance<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            account: value.account.into(),
            amount: value.amount.try_into()?,
        })
    }
}

impl std::fmt::Display for Balance<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.account.fmt(f)?;
        f.write_str(" ")?;
        self.amount.fmt(f)
    }
}

impl Balance<'_> {
    fn parse(input: &str) -> IResult<&str, Balance<'_>> {
        map(
            (terminated(Account::parse, tag(" ")), Amount::parse),
            |(account, amount)| Balance { account, amount },
        )
        .parse(input)
    }
}

impl Document<'_> {
    fn parse(input: &str) -> IResult<&str, Document<'_>> {
        map(
            (terminated(Account::parse, tag(" ")), double_quoted_string),
            |(account, path)| Document { account, path },
        )
        .parse(input)
    }
}

#[derive(Debug, Clone)]
struct Document<'src> {
    account: Account<'src>,
    path: DoubleQuotedString<'src>,
}

impl std::fmt::Display for Document<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.account.fmt(f)?;
        f.write_str(" ")?;
        self.path.fmt(f)
    }
}

#[derive(Debug, Clone)]
struct Transaction<'src> {
    flag: TransactionFlag,
    raw_narration: Option<&'src str>,
    rows: Vec<TransactionRow<'src>>,
}

impl TryFrom<Transaction<'_>> for crate::model::Transaction {
    type Error = rust_decimal::Error;

    fn try_from(tx: Transaction) -> Result<Self, Self::Error> {
        let mut rows = Vec::new();
        for r in tx.rows {
            rows.push(r.try_into()?);
        }
        Ok(Self {
            flag: tx.flag,
            narration: tx.raw_narration.unwrap_or_default().into(),
            second_narration: None,
            rows,
        })
    }
}

impl std::fmt::Display for Transaction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.flag.fmt(f)?;
        if let Some(raw_narration) = self.raw_narration {
            f.write_str(" ")?;
            f.write_str(raw_narration)?;
        }
        for row in &self.rows {
            f.write_str("\n  ")?;
            row.fmt(f)?;
        }
        Ok(())
    }
}

fn transaction(input: &str) -> IResult<&str, Transaction<'_>> {
    let flag = alt((
        value(TransactionFlag::Asterisk, tag("*")),
        value(TransactionFlag::Exclamation, tag("!")),
        value(TransactionFlag::Txn, tag("txn")),
    ));
    let narration = preceded(tag(" "), is_not("\n"));
    let row = preceded(tag("\n  "), transaction_row);
    map(
        (flag, opt(narration), many0(row)),
        |(flag, raw_narration, rows)| Transaction {
            flag,
            raw_narration,
            rows,
        },
    )
    .parse(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TransactionRow<'src> {
    flag: Option<&'src str>,
    account: Account<'src>,
    amount: Option<Amount<'src>>,
    unknown: Option<&'src str>,
}

impl TryFrom<TransactionRow<'_>> for crate::model::Posting {
    type Error = rust_decimal::Error;

    fn try_from(row: TransactionRow<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            flag: row.flag.map(|_| crate::model::TransactionRowFlag),
            account: row.account.into(),
            amount: row.amount.map(TryInto::try_into).transpose()?,
            rate: None,
        })
    }
}

impl std::fmt::Display for TransactionRow<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(flag) = self.flag {
            f.write_str(flag)?;
            f.write_str(" ")?;
        }
        self.account.fmt(f)?;
        if let Some(amount) = &self.amount {
            f.write_str(" ")?;
            amount.fmt(f)?;
        }
        if let Some(unknown) = self.unknown {
            f.write_str(" ")?;
            f.write_str(unknown)?;
        }
        Ok(())
    }
}

fn transaction_row(input: &str) -> IResult<&str, TransactionRow<'_>> {
    let flag = opt(terminated(tag("!"), tag(" ")));
    let amount = opt(preceded(tag(" "), Amount::parse));
    let unknown = opt(preceded(tag(" "), is_not("\n")));
    map(
        (flag, Account::parse, amount, unknown),
        |(flag, account, amount, unknown)| TransactionRow {
            flag,
            account,
            amount,
            unknown,
        },
    )
    .parse(input)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Amount<'src> {
    value: &'src str,
    unit: &'src str,
}

impl TryFrom<Amount<'_>> for crate::model::Amount {
    type Error = rust_decimal::Error;

    fn try_from(value: Amount<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            value: value.value.try_into()?,
            unit: crate::model::Currency(value.unit.into()),
        })
    }
}

impl std::fmt::Display for Amount<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.value)?;
        f.write_str(" ")?;
        f.write_str(self.unit)
    }
}

impl Amount<'_> {
    fn parse(input: &str) -> IResult<&str, Amount<'_>> {
        map(
            (is_not(" \n\t"), preceded(tag(" "), is_not(" \n\t"))),
            |(value, unit)| Amount { value, unit },
        )
        .parse(input)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Account<'src> {
    prefix: AccountPrefix,
    suffix: &'src str,
}

impl From<Account<'_>> for crate::model::Account {
    fn from(value: Account<'_>) -> Self {
        Self {
            prefix: value.prefix,
            components: value.suffix.split(':').skip(1).map(Into::into).collect(),
        }
    }
}

impl std::fmt::Display for Account<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.prefix.fmt(f)?;
        f.write_str(self.suffix)
    }
}

impl Account<'_> {
    fn parse(input: &str) -> IResult<&str, Account<'_>> {
        let prefix = alt((
            value(AccountPrefix::Assets, tag("Assets")),
            value(AccountPrefix::Liabilities, tag("Liabilities")),
            value(AccountPrefix::Equity, tag("Equity")),
            value(AccountPrefix::Income, tag("Income")),
            value(AccountPrefix::Expenses, tag("Expenses")),
        ));
        context(
            "account",
            map(
                (
                    prefix,
                    recognize(many0((tag(":"), Account::parse_component))),
                ),
                |(prefix, suffix)| Account { prefix, suffix },
            ),
        )
        .parse(input)
    }

    fn parse_component(input: &str) -> IResult<&str, &str> {
        let first_letter = is_a("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ");
        let folowing_letters =
            is_a("0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz-");
        recognize((first_letter, many0(folowing_letters))).parse(input)
    }
}

fn double_quoted_string(input: &str) -> IResult<&str, DoubleQuotedString<'_>> {
    // FIX: Escaped quote
    map(
        recognize((tag("\""), is_not("\n\""), tag("\""))),
        DoubleQuotedString,
    )
    .parse(input)
}

#[derive(Debug, Clone)]
struct DoubleQuotedString<'src>(&'src str);

impl std::fmt::Display for DoubleQuotedString<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl DoubleQuotedString<'_> {
    fn to_value(&self) -> String {
        self.0
            .strip_prefix('"')
            .unwrap()
            .strip_suffix('"')
            .unwrap()
            .into()
    }
}

#[derive(Debug, Clone)]
struct Open<'src> {
    account: Account<'src>,
    currencies: Vec<&'src str>,
}

impl Open<'_> {
    fn parse(input: &str) -> IResult<&str, Open<'_>> {
        map(
            (Account::parse, many0(preceded(tag(" "), is_not(" \n\t")))),
            |(account, currencies)| Open {
                account,
                currencies,
            },
        )
        .parse(input)
    }
}

impl std::fmt::Display for Open<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.account.fmt(f)?;
        for c in &self.currencies {
            f.write_str(" ")?;
            c.fmt(f)?;
        }
        Ok(())
    }
}

impl From<Open<'_>> for crate::model::Open {
    fn from(value: Open<'_>) -> Self {
        Self {
            account: value.account.into(),
            currencies: value
                .currencies
                .into_iter()
                .map(|c| crate::model::Currency(c.into()))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
struct Pad<'src> {
    account: Account<'src>,
    account_pad: Account<'src>,
}

impl From<Pad<'_>> for crate::model::Pad {
    fn from(value: Pad) -> Self {
        Self {
            account: value.account.into(),
            account_pad: value.account_pad.into(),
        }
    }
}

impl Pad<'_> {
    fn parse(input: &str) -> IResult<&str, Pad<'_>> {
        map(
            (Account::parse, Account::parse),
            |(account, account_pad)| Pad {
                account,
                account_pad,
            },
        )
        .parse(input)
    }
}

impl std::fmt::Display for Pad<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.account.fmt(f)?;
        self.account_pad.fmt(f)
    }
}

#[derive(Debug, Clone)]
struct Price<'src> {
    currency: &'src str,
    amount: Amount<'src>,
}

impl Price<'_> {
    fn parse(input: &str) -> IResult<&str, Price<'_>> {
        map((is_not(" \n\t"), Amount::parse), |(currency, amount)| {
            Price { currency, amount }
        })
        .parse(input)
    }
}

impl std::fmt::Display for Price<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.currency)?;
        self.amount.fmt(f)
    }
}

#[cfg(test)]
mod tests {

    use super::{Account, AccountPrefix, Amount, Date, DirectiveInner, TransactionRow, directive};

    #[test]
    fn parse_sparse_transaction() {
        let (rest, directive) = directive("2020-01-01 *").unwrap();
        assert_eq!(directive.date, Date::new(2020, 1, 1).unwrap());

        let tx = match directive.inner {
            DirectiveInner::Transaction(tx) => tx,
            v => {
                panic!("not a transaction: {v:?}")
            }
        };
        assert_eq!(tx.raw_narration, None);
        assert!(tx.rows.is_empty());
        assert_eq!(rest, "");
    }

    #[test]
    fn parse_basic_transaction() {
        let (rest, directive) =
            directive("2020-01-01 * \"Testing\"\n  Assets:Foo 1 USD\n  Assets:Bar").unwrap();
        assert_eq!(directive.date, Date::new(2020, 1, 1).unwrap());

        let DirectiveInner::Transaction(tx) = directive.inner else {
            panic!("not a transaction")
        };
        assert_eq!(tx.raw_narration, Some("\"Testing\""));
        assert_eq!(
            tx.rows,
            vec![
                TransactionRow {
                    flag: None,
                    account: Account {
                        prefix: AccountPrefix::Assets,
                        suffix: ":Foo",
                    },
                    amount: Some(Amount {
                        value: "1",
                        unit: "USD",
                    }),
                    unknown: None,
                },
                TransactionRow {
                    flag: None,
                    account: Account {
                        prefix: AccountPrefix::Assets,
                        suffix: ":Bar",
                    },
                    amount: None,
                    unknown: None,
                }
            ]
        );
        assert_eq!(rest, "");
    }

    #[test]
    fn parse_comment() {
        super::statements("; abc").unwrap();
    }

    #[test]
    fn parse_balance() {
        super::statements("2020-01-01 balance Assets:Foo 200 USD").unwrap();
    }

    #[test]
    fn parse_transaction_metadata() {
        super::statements(
            "2020-01-01 * \"Hi\"\n  Assets:Foo 1 USD\n  Assets:Bar\n  custom: \"Hello\"",
        )
        .unwrap();
    }

    #[test]
    fn parse_balance_metadata() {
        super::statements("2020-01-01 balance Assets:Foo 200 USD\n  custom: \"Hello\"").unwrap();
    }
}
