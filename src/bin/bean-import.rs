use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    str::FromStr as _,
};

use beancount_rs::{
    date::Date,
    model::{
        Account, AccountPrefix, Amount, Balance, Currency, Directive, DirectiveInner, Document,
        MetadataEntry, Posting, Transaction, TransactionFlag,
    },
};
use clap::{Parser, ValueEnum};
use pdfium::{PdfiumDocument, PdfiumRect};
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Parser)]
struct Cli {
    input: PathBuf,

    account_name: String,

    #[arg(value_enum)]
    input_format: InputFormatKind,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InputFormatKind {
    HSBCPdf,
    ChaseCsv,
}

fn main() {
    let cli = Cli::parse();

    let bank_account: Box<dyn Format> = match cli.input_format {
        InputFormatKind::HSBCPdf => Box::new(HSBCPdf {
            account: cli.account_name.parse().unwrap(),
            unknown: Account {
                prefix: AccountPrefix::Expenses,
                components: vec!["Unknown".into()],
            },
            currency: Currency("GBP".into()),
        }),
        InputFormatKind::ChaseCsv => Box::new(ChaseCsv {
            account: cli.account_name.parse().unwrap(),
            unknown: Account {
                prefix: AccountPrefix::Expenses,
                components: vec!["Unknown".into()],
            },
        }),
    };
    let directives = bank_account.read_file(&cli.input);
    for d in directives {
        println!("{d}\n");
    }
}

fn parse_amount(s: &str, unit: &Currency) -> Option<Amount> {
    if s.is_empty() {
        return None;
    }
    let value: Decimal = s.parse().unwrap();
    Some(Amount {
        value,
        unit: unit.clone(),
    })
}

fn divide_row(header_column_left_rights: &[(f32, f32)], row: &[PdfWord]) -> Vec<String> {
    let mut out = Vec::new();
    let mut lrs = header_column_left_rights.iter();
    // Skip first column
    lrs.next().unwrap();
    let mut left = lrs.next().copied().unwrap().0;
    let mut acc = String::new();
    for w in row {
        #[expect(clippy::while_float)]
        while w.rect.right > left {
            out.push(acc);
            acc = String::new();
            if let Some((l, _)) = lrs.next().copied() {
                left = l;
            } else {
                break;
            }
        }
        if !acc.is_empty() {
            acc.push(' ');
        }
        acc.push_str(&w.contents);
    }
    out.push(acc);
    while out.len() < header_column_left_rights.len() {
        out.push(String::new());
    }
    out
}

fn get_column_left_rights(headers: &[&str], mut header_row: &[PdfWord]) -> Vec<(f32, f32)> {
    let mut left_rights = Vec::new();
    for &h in headers {
        let left = header_row[0].rect.left;
        let mut right = left;
        for mut h_word in h.split(' ') {
            while let Some(stripped) = h_word.strip_prefix(&header_row[0].contents) {
                right = header_row[0].rect.right;
                header_row = &header_row[1..];
                if stripped.is_empty() {
                    break;
                }
                h_word = stripped;
            }
        }
        left_rights.push((left, right));
    }
    left_rights
}

fn find_header_row(headers: &[&str], word_rows: &[Vec<PdfWord>]) -> Option<usize> {
    let header_compact: String = headers.join("").replace(' ', "");
    word_rows.iter().position(|row| {
        let row_compact: String = row.iter().map(|w| w.contents.clone()).collect();
        header_compact == row_compact
    })
}

fn collect_word_rows(chars: Vec<PdfChar>) -> Vec<Vec<PdfWord>> {
    let rows = group_into_rows(chars);
    let mut word_rows: Vec<Vec<PdfWord>> = Vec::new();
    for mut row in rows {
        row.sort_by(|a, b| f32::total_cmp(&a.rect.left, &b.rect.left));
        let avg_width = calc_avg_width(&row);
        let mut words = Vec::new();
        let mut contents = String::new();
        let mut rect = PdfiumRect::new(
            f32::INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::NEG_INFINITY,
        );
        let mut prev_x: Option<f32> = None;
        for c in row {
            if let Some(px) = prev_x {
                let advance = c.rect.left - px;
                if advance > avg_width * 0.4 {
                    prev_x.take().unwrap();
                    words.push(PdfWord { contents, rect });
                    contents = String::new();
                    rect = PdfiumRect::new(
                        f32::INFINITY,
                        f32::INFINITY,
                        f32::NEG_INFINITY,
                        f32::NEG_INFINITY,
                    );
                }
            }
            rect.left = rect.left.min(c.rect.left);
            rect.right = rect.right.max(c.rect.right);
            rect.top = rect.top.min(c.rect.top);
            rect.bottom = rect.bottom.max(c.rect.bottom);
            contents.push(c.c);
            prev_x = Some(rect.right);
        }
        words.push(PdfWord { contents, rect });
        word_rows.push(words);
    }
    word_rows
}

#[derive(Debug)]
struct PdfChar {
    c: char,
    rect: PdfiumRect,
}

#[derive(Debug)]
struct PdfWord {
    contents: String,
    rect: PdfiumRect,
}

fn extract_visible_chars(page: &pdfium::PdfiumPage) -> Vec<PdfChar> {
    let text = page.text().unwrap();
    let mut out = Vec::new();
    for i in 0..text.char_count().unwrap() {
        let c = text.extract(i, 1).chars().next().unwrap();
        if c.is_whitespace() {
            continue;
        }
        let rect = text.get_char_box(i).unwrap();
        out.push(PdfChar { c, rect });
    }
    out
}

fn group_into_rows(mut chars: Vec<PdfChar>) -> Vec<Vec<PdfChar>> {
    chars.sort_by(|a, b| {
        f32::total_cmp(
            &f32::midpoint(b.rect.top, b.rect.bottom),
            &f32::midpoint(a.rect.top, a.rect.bottom),
        )
    });

    let avg_height = calc_avg_height(&chars);

    let mut rows: Vec<Vec<PdfChar>> = Vec::new();
    let mut row_anchor_y = f32::NAN;

    for item in chars {
        let row_avg_height = rows
            .last()
            .and_then(|r| {
                if r.len() > 4 {
                    Some(calc_avg_height(r))
                } else {
                    None
                }
            })
            .unwrap_or(avg_height);
        if rows.is_empty() || (row_anchor_y - item.rect.bottom).abs() > row_avg_height * 0.5 {
            rows.push(Vec::new());
            row_anchor_y = item.rect.bottom;
        }
        rows.last_mut().unwrap().push(item);
    }

    rows
}

fn calc_avg_width(chars: &[PdfChar]) -> f32 {
    let total_height: f32 = chars.iter().map(|c| c.rect.width()).sum();
    let count = chars.len();
    total_height / count as f32
}

fn calc_avg_height(chars: &[PdfChar]) -> f32 {
    let total_height: f32 = chars.iter().map(|c| c.rect.height()).sum();
    let count = chars.len();
    total_height / count as f32
}

trait Format {
    fn read_file(&self, path: &Path) -> Vec<Directive>;
}

struct HSBCPdf {
    account: Account,
    currency: Currency,
    unknown: Account,
}

impl Format for HSBCPdf {
    fn read_file(&self, path: &Path) -> Vec<Directive> {
        let document = pdfium::PdfiumDocument::new_from_path(path, None).unwrap();
        let mut directives = self.parse(&document);

        directives.sort_by_key(|d| d.date);
        self.document(path, &mut directives);
        directives
    }
}

impl HSBCPdf {
    fn parse(&self, document: &PdfiumDocument) -> Vec<Directive> {
        let table_headers = [
            "Date",
            "Payment type and details",
            "Paid out",
            "Paid in",
            "Balance",
        ];
        let mut directives = Vec::new();
        for (index, page) in document.pages().enumerate() {
            let page = page.unwrap();
            let chars = extract_visible_chars(&page);
            if chars.is_empty() {
                println!("Page {}: no extractable text, skipping.", index + 1);
                continue;
            }

            let word_rows = collect_word_rows(chars);
            let Some(header_row_index) = find_header_row(&table_headers, &word_rows) else {
                continue;
            };
            let header_column_left_rights =
                get_column_left_rights(&table_headers, &word_rows[header_row_index]);
            let mut rows = Vec::new();
            for r in &word_rows[header_row_index + 1..] {
                let row = divide_row(&header_column_left_rights, r);
                rows.push(row);
            }
            self.parse_rows(rows, &mut directives);
        }
        directives
    }

    fn parse_rows(&self, rows: Vec<Vec<String>>, directives: &mut Vec<Directive>) {
        let mut new_rows = Vec::<Vec<String>>::new();
        for row in rows {
            if row[0] == "A" {
                continue;
            }
            if row[0].starts_with("67 George") {
                continue;
            }
            new_rows.push(row);
        }
        let mut date = None;
        let mut prev_narr = None;
        for r in new_rows {
            if r[1].starts_with("BALANCE BROUGHT FORWARD")
                || r[1].starts_with("BALANCE CARRIED FORWARD")
            {
                continue;
            }
            if !r[0].is_empty() {
                date = Some(Date::parse_uk(&r[0]).unwrap());
            }
            if !r[4].is_empty() {
                directives.push(Directive {
                    date: date.unwrap().next_day(),
                    inner: DirectiveInner::Balance(Balance {
                        account: self.account.clone(),
                        amount: parse_amount(&r[4], &self.currency).unwrap(),
                    }),
                    metadata: Vec::new(),
                });
            }
            if r[2].is_empty() && r[3].is_empty() {
                prev_narr = Some(r[1].clone());
                continue;
            }
            let rows = vec![
                Posting {
                    flag: None,
                    account: self.account.clone(),
                    amount: parse_amount(&r[3], &self.currency),
                    rate: None,
                },
                Posting {
                    flag: None,
                    account: self.unknown.clone(),
                    amount: parse_amount(&r[2], &self.currency),
                    rate: None,
                },
            ];
            directives.push(Directive {
                date: date.unwrap(),
                inner: DirectiveInner::Transaction(Transaction {
                    flag: TransactionFlag::Exclamation,
                    narration: prev_narr.take().map(|s| s + " ").unwrap_or_default() + &r[1],
                    second_narration: None,
                    rows,
                }),
                metadata: Vec::new(),
            });
        }
    }

    fn document(&self, path: &Path, directives: &mut Vec<Directive>) {
        directives.push(Directive {
            date: directives.last().unwrap().date,
            inner: DirectiveInner::Document(Document {
                account: self.account.clone(),
                path: path.to_string_lossy().to_string(),
            }),
            metadata: Vec::new(),
        });
    }
}

#[derive(Debug, Deserialize)]
struct Record {
    #[serde(rename = "Date")]
    date: String,
    #[serde(rename = "Time")]
    time: String,
    #[serde(rename = "Transaction Type")]
    transaction_type: String,
    #[serde(rename = "Transaction Description")]
    transaction_description: String,
    #[serde(rename = "Amount")]
    amount: String,
    #[serde(rename = "Currency")]
    currency: String,
    #[serde(rename = "Balance")]
    balance: String,
}

struct ChaseCsv {
    account: Account,
    unknown: Account,
}

impl Format for ChaseCsv {
    #[expect(clippy::too_many_lines)]
    fn read_file(&self, path: &Path) -> Vec<Directive> {
        let re = regex::Regex::new(r"[\.\d]+$").unwrap();
        let categories: Categories =
            toml::from_str(&std::fs::read_to_string("categories.toml").unwrap()).unwrap();
        let raw_data = std::fs::read(path).unwrap();
        let decoded: Result<Vec<u8>, _> = std::str::from_utf8(&raw_data)
            .unwrap()
            .chars()
            .map(|c| u8::try_from(c as u32))
            .collect();
        let decoded = String::from_utf8(decoded.unwrap())
            .unwrap()
            .replace("&amp;", "&");
        let (_, decoded) = decoded.split_once("\r\n").unwrap();
        let mut rdr = csv::Reader::from_reader(std::io::Cursor::new(decoded));
        let mut directives = Vec::new();
        for result in rdr.deserialize() {
            let record: Record = result.unwrap();
            let amount = record.amount.replace(',', "").parse().unwrap();
            let mut rows = Vec::new();
            if record.transaction_type.contains(" FX rate") {
                let (_, suffix) = record.transaction_type.split_once(" | ").unwrap();
                let (fx_amount, fx) = suffix.split_once(" | ").unwrap();
                let (currency, fx_amount) = fx_amount.split_once(' ').unwrap();
                let (_, fx) = fx.split_once(" = ").unwrap();
                let fx_match = re.find(fx).unwrap();
                let fx_match = fx_match.as_str();
                rows.push(Posting {
                    flag: None,
                    account: self.account.clone(),
                    amount: Some(Amount {
                        value: amount,
                        unit: Currency(record.currency.clone()),
                    }),
                    rate: Some(format!("{fx_match} {currency}")),
                });
                rows.push(Posting {
                    flag: None,
                    account: self.unknown.clone(),
                    amount: Some(Amount {
                        value: fx_amount.parse().unwrap(),
                        unit: Currency(currency.to_owned()),
                    }),
                    rate: None,
                });
            } else {
                rows.push(Posting {
                    flag: None,
                    account: self.account.clone(),
                    amount: Some(Amount {
                        value: amount,
                        unit: Currency(record.currency.clone()),
                    }),
                    rate: None,
                });
                rows.push(Posting {
                    flag: None,
                    account: self.unknown.clone(),
                    amount: Some(Amount {
                        value: -amount,
                        unit: Currency(record.currency.clone()),
                    }),
                    rate: None,
                });
            }

            directives.push(Directive {
                date: Date::parse_uk(&record.date).unwrap(),
                inner: DirectiveInner::Transaction(Transaction {
                    flag: TransactionFlag::Asterisk,
                    narration: record.transaction_description,
                    second_narration: Some(record.transaction_type),
                    rows,
                }),
                metadata: vec![MetadataEntry {
                    key: String::from("datetime"),
                    value: format!("{} {}", record.time, record.date),
                }],
            });
            directives.push(Directive {
                date: Date::parse_uk(&record.date).unwrap().next_day(),
                inner: DirectiveInner::Balance(Balance {
                    account: self.account.clone(),
                    amount: Amount {
                        value: record.balance.replace(',', "").parse().unwrap(),
                        unit: Currency(record.currency.clone()),
                    },
                }),
                metadata: vec![MetadataEntry {
                    key: String::from("datetime"),
                    value: format!("{} {}", record.time, record.date),
                }],
            });
        }
        directives.push(Directive {
            date: directives.last().unwrap().date,
            inner: DirectiveInner::Document(Document {
                account: self.account.clone(),
                path: path.display().to_string(),
            }),
            metadata: Vec::new(),
        });
        for d in &mut directives {
            if let DirectiveInner::Transaction(tx) = &mut d.inner {
                categories.categorise_transaction(tx);
            }
        }
        directives.reverse();
        let mut last_balance_date = None;
        directives.retain(|d| match &d.inner {
            DirectiveInner::Balance(_) => {
                if Some(d.date) == last_balance_date {
                    false
                } else {
                    last_balance_date = Some(d.date);
                    true
                }
            }
            _ => true,
        });
        directives.reverse();
        directives
    }
}

#[derive(serde::Deserialize)]
struct Categories {
    entries: HashMap<String, String>,
}

impl Categories {
    fn categorise_transaction(&self, tx: &mut Transaction) {
        if let Some(account) = self.entries.get(&tx.narration) {
            tx.rows[1].account = Account::from_str(account).unwrap();
        }
    }
}
