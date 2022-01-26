use aurora_engine_transactions::{
    legacy::{LegacyEthSignedTransaction, TransactionLegacy},
    EthTransactionKind,
};
use aurora_engine_types::types::{Address, Wei};
use aurora_engine_types::{H256, U256};
use std::convert::TryFrom;
use std::io::{Cursor, Read};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionParams {
    // URL to the host (eg localhost)
    pub host: String,
    pub port: u32,
    pub db_name: String,
    pub user: String,
    pub password: String,
}

impl ConnectionParams {
    pub fn as_connection_string(&self) -> String {
        format!(
            "host={} port={} dbname={} user={} password={}",
            self.host, self.port, self.db_name, self.user, self.password
        )
    }
}

impl Default for ConnectionParams {
    fn default() -> Self {
        Self {
            host: "localhost".into(),
            port: 15432,
            db_name: "aurora".into(),
            user: "aurora".into(),
            password: "aurora".into(),
        }
    }
}

/// Row from the `block` table in the relayer's DB.
#[derive(Debug)]
pub struct BlockRow {
    /// Chain ID the block is from
    pub chain: u64,
    /// Block height
    pub id: u64,
    /// Block hash (on Aurora)
    pub hash: H256,
    /// Block hash (on NEAR)
    pub near_hash: Option<H256>,
    /// Time the block was created (in ns since the unix epoch)
    pub timestamp: Option<u64>,
    /// Size of the block (in bytes)
    pub size: u32,
    /// Maximum amount of EVM gas allowed to be spent
    pub gas_limit: U256,
    /// Amount of EVM gas spent in transactions in this block
    pub gas_used: U256,
    /// Hash of the parent block
    pub parent_hash: H256,
    /// Root hash for transactions trie
    pub transactions_root: H256,
    /// Root hash for state trie
    pub state_root: H256,
    /// Root hash for receipts trie
    pub receipts_root: H256,
}

impl From<postgres::Row> for BlockRow {
    fn from(row: postgres::Row) -> Self {
        let chain: i32 = row.get("chain");
        let id: i64 = row.get("id");
        let hash = get_hash(&row, "hash");
        let near_hash: Option<&[u8]> = row.get("near_hash");
        let timestamp = get_timestamp(&row, "timestamp");
        let size: i32 = row.get("size");
        let gas_limit = get_numeric(&row, "gas_limit");
        let gas_used = get_numeric(&row, "gas_used");
        let parent_hash = get_hash(&row, "parent_hash");
        let transactions_root = get_hash(&row, "transactions_root");
        let state_root = get_hash(&row, "state_root");
        let receipts_root = get_hash(&row, "receipts_root");

        Self {
            chain: chain as u64,
            id: id as u64,
            hash,
            near_hash: near_hash.map(H256::from_slice),
            timestamp,
            size: size as u32,
            gas_limit,
            gas_used,
            parent_hash,
            transactions_root,
            state_root,
            receipts_root,
        }
    }
}

/// Row from the `transaction` table in the relayer's DB.
#[derive(Debug)]
pub struct TransactionRow {
    /// Block height where the transaction was included in the chain
    pub block: u64,
    /// Hash of the block which included the transaction. Not present in the `transaction` table, so will need
    /// to be filled using a `JOIN` against the `block` table.
    pub block_hash: H256,
    /// Position in the block (if a block includes multiple transactions this index will increase)
    pub index: u16,
    /// Some unique id?
    pub id: u64,
    /// Transaction hash (on Aurora)
    pub hash: H256,
    /// Transaction hash (on NEAR)
    pub near_hash: H256,
    /// Hash of the receipt on NEAR that the transaction was processed in
    pub near_receipt_hash: H256,
    /// Address that signed the transaction
    pub from: Address,
    /// Address the transaction is sent to
    pub to: Option<Address>,
    /// Nonce of the transaction
    pub nonce: U256,
    /// Gas price
    pub gas_price: U256,
    /// Maximum amount of EVM gas the transaction can spend
    pub gas_limit: U256,
    /// Amount of EVM gas used in the transaction
    pub gas_used: u64,
    /// Value attached to the transaction
    pub value: Wei,
    /// Input sent with the transaction
    pub input: Vec<u8>,
    /// Signature parameter v
    pub v: u64,
    /// Signature parameter r
    pub r: U256,
    /// Signature parameter s
    pub s: U256,
    /// True if transaction succeeded
    pub status: bool,
    /// Output bytes from the transaction execution
    pub output: Vec<u8>,
}

impl From<postgres::Row> for TransactionRow {
    fn from(row: postgres::Row) -> Self {
        let block: i64 = row.get("block");
        let block_hash = get_hash(&row, "block_hash");
        let index: i32 = row.get("index");
        let id: i64 = row.get("id");
        let hash = get_hash(&row, "hash");
        let near_hash = get_hash(&row, "near_hash");
        let near_receipt_hash = get_hash(&row, "near_receipt_hash");
        let from = get_address(&row, "from");
        let to: Option<&[u8]> = row.get("to");
        let nonce = get_numeric(&row, "nonce");
        let gas_price = get_numeric(&row, "gas_price");
        let gas_limit = get_numeric(&row, "gas_limit");
        let gas_used = get_numeric(&row, "gas_used");
        let value = get_numeric(&row, "value");
        let input: Option<Vec<u8>> = row.get("input");
        let v = get_numeric(&row, "v");
        let r = get_numeric(&row, "r");
        let s = get_numeric(&row, "s");
        let status: bool = row.get("status");
        let output: Option<Vec<u8>> = row.get("output");

        Self {
            block: block as u64,
            block_hash,
            index: index as u16,
            id: id as u64,
            hash,
            near_hash,
            near_receipt_hash,
            from,
            to: to.map(|arr| Address::try_from_slice(arr).unwrap()),
            nonce,
            gas_price,
            gas_limit,
            gas_used: gas_used.low_u64(),
            value: Wei::new(value),
            input: input.unwrap_or_default(),
            v: v.low_u64(),
            r,
            s,
            status,
            output: output.unwrap_or_default(),
        }
    }
}

impl From<TransactionRow> for EthTransactionKind {
    fn from(row: TransactionRow) -> Self {
        let legacy = LegacyEthSignedTransaction {
            transaction: TransactionLegacy {
                nonce: row.nonce,
                gas_price: row.gas_price,
                gas_limit: row.gas_limit,
                to: row.to,
                value: row.value,
                data: row.input,
            },
            v: row.v,
            r: row.r,
            s: row.s,
        };

        Self::Legacy(legacy)
    }
}

fn get_numeric(row: &postgres::Row, field: &str) -> U256 {
    let value: PostgresNumeric = row.get(field);
    U256::try_from(value).unwrap()
}

fn get_hash(row: &postgres::Row, field: &str) -> H256 {
    let value: &[u8] = row.get(field);
    H256::from_slice(value)
}

fn get_address(row: &postgres::Row, field: &str) -> Address {
    let value: &[u8] = row.get(field);
    Address::try_from_slice(value).unwrap()
}

fn get_timestamp(row: &postgres::Row, field: &str) -> Option<u64> {
    let timestamp: Option<SystemTime> = row.get(field);
    timestamp
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
}

struct PostgresNumeric {
    /// The contribution of the first group to the value of the number is given by `groups[0] * 10000^weight`.
    /// The weight decreases by 1 for subsequent groups
    weight: i16,
    /// Sign of the number
    sign: PostgresNumericSign,
    /// The number of base10 digits to put after the decimal separator
    scale: u16,
    /// The "digits" of the number in base 10000 (offset by the weight).
    groups: Vec<u16>,
}

impl PostgresNumeric {
    const BASE_WEIGHT: U256 = U256([10000u64, 0, 0, 0]);
}

#[repr(u16)]
enum PostgresNumericSign {
    Positive = 0x0000,
    Negative = 0x4000,
    NaN = 0xc000,
}

impl TryFrom<PostgresNumeric> for U256 {
    type Error = NumericToU256Error;

    fn try_from(value: PostgresNumeric) -> Result<Self, Self::Error> {
        if let PostgresNumericSign::Negative = value.sign {
            return Err(NumericToU256Error::Negative);
        } else if let PostgresNumericSign::NaN = value.sign {
            return Err(NumericToU256Error::NaN);
        } else if value.scale != 0 || value.weight < 0 {
            return Err(NumericToU256Error::NotAWholeNumber);
        }

        let mut total = U256::zero();
        let mut weight = PostgresNumeric::BASE_WEIGHT
            .checked_pow(value.weight.into())
            .ok_or(NumericToU256Error::Overflow)?;
        for group in value.groups {
            let contribution = U256::from(group)
                .checked_mul(weight)
                .ok_or(NumericToU256Error::Overflow)?;
            total = total
                .checked_add(contribution)
                .ok_or(NumericToU256Error::Overflow)?;
            weight /= PostgresNumeric::BASE_WEIGHT;
        }
        Ok(total)
    }
}

#[derive(Debug)]
enum NumericToU256Error {
    Negative,
    NaN,
    NotAWholeNumber,
    Overflow,
}

impl<'a> postgres::types::FromSql<'a> for PostgresNumeric {
    fn from_sql(
        _: &postgres::types::Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let mut cursor = Cursor::new(raw);
        let read_16bits = |cursor: &mut Cursor<&[u8]>| -> Result<[u8; 2], std::io::Error> {
            let mut buf = [0u8; 2];
            cursor.read_exact(&mut buf)?;
            Ok(buf)
        };
        let read_u16 = |cursor: &mut Cursor<&[u8]>| -> Result<u16, std::io::Error> {
            read_16bits(cursor).map(u16::from_be_bytes)
        };
        let read_i16 = |cursor: &mut Cursor<&[u8]>| -> Result<i16, std::io::Error> {
            read_16bits(cursor).map(i16::from_be_bytes)
        };

        let num_groups = read_u16(&mut cursor)?;
        let weight = read_i16(&mut cursor)?;

        let sign_raw = read_u16(&mut cursor)?;
        let sign = if sign_raw == PostgresNumericSign::Positive as u16 {
            PostgresNumericSign::Positive
        } else if sign_raw == PostgresNumericSign::Negative as u16 {
            PostgresNumericSign::Negative
        } else if sign_raw == PostgresNumericSign::NaN as u16 {
            PostgresNumericSign::NaN
        } else {
            panic!("Unexpected Numeric Sign value");
        };

        let scale = read_u16(&mut cursor)?;
        let mut groups = Vec::with_capacity(num_groups as usize);
        for _ in 0..num_groups {
            groups.push(read_u16(&mut cursor)?);
        }

        Ok(PostgresNumeric {
            weight,
            sign,
            scale,
            groups,
        })
    }

    fn accepts(ty: &postgres::types::Type) -> bool {
        matches!(ty, &postgres::types::Type::NUMERIC)
    }
}
