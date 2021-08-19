use borsh::BorshDeserialize;
use ethabi::{encode, Token as ABIToken};
use logos::Logos;
use rlp::{Decodable, DecoderError, Rlp};

use crate::parameters::MetaCallArgs;
use crate::prelude::{vec, Address, Box, HashMap, String, ToOwned, ToString, Vec, H256, U256};
use crate::types::{keccak, u256_to_arr, InternalMetaCallArgs, RawU256, Wei};

/// Internal errors to propagate up and format in the single place.
pub enum ParsingError {
    ArgumentParseError,
    InvalidMetaTransactionMethodName,
    InvalidMetaTransactionFunctionArg,
    InvalidEcRecoverSignature,
    ArgsLengthMismatch,
}

pub type ParsingResult<T> = core::result::Result<T, ParsingError>;

mod type_lexer {
    use logos::{Lexer, Logos};

    #[derive(Logos, Debug, PartialEq)]
    pub(super) enum Token {
        #[regex("byte|bytes[1-2][0-9]?|bytes3[0-2]?|bytes[4-9]", fixed_bytes_size)]
        FixedBytes(u8),
        #[regex("uint(8|16|24|32|40|48|56|64|72|80|88|96|104|112|120|128|136|144|152|160|168|176|184|192|200|208|216|224|232|240|248|256)?", |lex| fixed_int_size(lex, "uint"))]
        Uint(usize),
        #[regex("int(8|16|24|32|40|48|56|64|72|80|88|96|104|112|120|128|136|144|152|160|168|176|184|192|200|208|216|224|232|240|248|256)?", |lex| fixed_int_size(lex, "int"))]
        Int(usize),
        #[regex("bool")]
        Bool,
        #[regex("address")]
        Address,
        #[regex("bytes")]
        Bytes,
        #[regex("string")]
        String,
        #[regex("\\[[0-9]*\\]", reference_type_size)]
        ReferenceType(Option<u64>),
        #[regex("[a-zA-Z_$][a-zA-Z0-9_$]*")]
        Identifier,

        #[error]
        Error,
    }

    fn fixed_bytes_size(lex: &mut Lexer<Token>) -> u8 {
        let slice = lex.slice();

        if slice == "byte" {
            return 1;
        }

        let n = slice["bytes".len()..].parse();
        n.ok().unwrap_or(1)
    }

    fn fixed_int_size(lex: &mut Lexer<Token>, prefix: &str) -> usize {
        let slice = lex.slice();

        if slice == prefix {
            // the default int size is 32
            return 32;
        }

        let n = slice[prefix.len()..].parse();
        n.unwrap_or(32)
    }

    fn reference_type_size(lex: &mut Lexer<Token>) -> Option<u64> {
        let slice = lex.slice();

        if slice == "[]" {
            return None;
        }

        let end_index = slice.len() - 1;
        let n = slice[1..end_index].parse();
        n.ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgType {
    Address,
    Uint,
    Int,
    String,
    Bool,
    Bytes,
    Byte(u8),
    Custom(String),
    Array {
        length: Option<u64>,
        inner: Box<ArgType>,
    },
}

/// the type string is being validated before it's parsed.
/// field_type: A single evm function arg type in string, without the argument name
/// e.g. "bytes" "uint256[][3]" "CustomStructName"
pub fn parse_type(field_type: &str) -> ParsingResult<ArgType> {
    let mut lexer = type_lexer::Token::lexer(field_type);
    let mut current_token = lexer.next();
    let mut inner_type: Option<ArgType> = None;

    loop {
        let typ = match current_token {
            None => break,
            Some(type_lexer::Token::Address) => ArgType::Address,
            Some(type_lexer::Token::Bool) => ArgType::Bool,
            Some(type_lexer::Token::String) => ArgType::String,
            Some(type_lexer::Token::Bytes) => ArgType::Bytes,
            Some(type_lexer::Token::Identifier) => ArgType::Custom(lexer.slice().to_owned()),
            Some(type_lexer::Token::FixedBytes(size)) => ArgType::Byte(size),
            Some(type_lexer::Token::Int(_)) => ArgType::Int,
            Some(type_lexer::Token::Uint(_)) => ArgType::Uint,
            Some(type_lexer::Token::ReferenceType(length)) => match inner_type {
                None => return Err(ParsingError::ArgumentParseError),
                Some(t) => ArgType::Array {
                    length,
                    inner: Box::new(t),
                },
            },
            Some(type_lexer::Token::Error) => return Err(ParsingError::ArgumentParseError),
        };
        inner_type = Some(typ);
        current_token = lexer.next();
    }

    inner_type.ok_or(ParsingError::ArgumentParseError)
}

/// NEAR's domainSeparator
/// See https://eips.ethereum.org/EIPS/eip-712#definition-of-domainseparator
/// and https://eips.ethereum.org/EIPS/eip-712#rationale-for-domainseparator
/// for definition and rationale for domainSeparator.
pub fn near_erc712_domain(chain_id: U256) -> RawU256 {
    let mut bytes = Vec::with_capacity(70);
    bytes.extend_from_slice(
        keccak("EIP712Domain(string name,string version,uint256 chainId)".as_bytes()).as_bytes(),
    );
    let near: RawU256 = keccak(b"NEAR").into();
    bytes.extend_from_slice(&near);
    let version: RawU256 = keccak(b"1").into();
    bytes.extend_from_slice(&version);
    bytes.extend_from_slice(&u256_to_arr(&chain_id));
    keccak(&bytes).into()
}

/// method_sig: format like "adopt(uint256,PetObj)" (no additional PetObj definition)
pub fn method_sig_to_abi(method_sig: &str) -> [u8; 4] {
    let mut result = [0u8; 4];
    result.copy_from_slice(&keccak(method_sig.as_bytes())[..4]);
    result
}

pub fn encode_address(addr: Address) -> Vec<u8> {
    let mut bytes = vec![0u8; 12];
    bytes.extend_from_slice(&addr.0);
    bytes
}

pub fn encode_string(s: &str) -> Vec<u8> {
    let mut bytes = vec![];
    bytes.extend_from_slice(keccak(s.as_bytes()).as_bytes());
    bytes
}

#[derive(Debug, Eq, PartialEq)]
pub enum RlpValue {
    Bytes(Vec<u8>),
    List(Vec<RlpValue>),
}

impl Decodable for RlpValue {
    fn decode(rlp: &Rlp<'_>) -> core::result::Result<Self, DecoderError> {
        if rlp.is_list() {
            Ok(RlpValue::List(rlp.as_list()?))
        } else {
            Ok(RlpValue::Bytes(
                rlp.decoder().decode_value(|bytes| Ok(bytes.to_vec()))?,
            ))
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
/// An argument specified in a evm method definition
pub struct Arg {
    #[allow(dead_code)]
    pub name: String,
    pub type_raw: String,
    pub t: ArgType,
}

#[derive(Debug, Eq, PartialEq)]
/// A parsed evm method definition
pub struct Method {
    pub name: String,
    pub raw: String,
    pub args: Vec<Arg>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct MethodAndTypes {
    pub method: Method,
    pub type_sequences: Vec<String>,
    pub types: HashMap<String, Method>,
}

impl Arg {
    fn parse(text: &str) -> ParsingResult<(Arg, &str)> {
        let (type_raw, remains) = parse_type_raw(text)?;
        let t = parse_type(&type_raw)?;
        let remains = consume(remains, ' ')?;
        let (name, remains) = parse_ident(remains)?;
        Ok((Arg { name, type_raw, t }, remains))
    }

    fn parse_args(text: &str) -> ParsingResult<(Vec<Arg>, &str)> {
        let mut remains = consume(text, '(')?;
        if remains.is_empty() {
            return Err(ParsingError::InvalidMetaTransactionMethodName);
        }
        let mut args = vec![];
        let first = remains.chars().next().unwrap();
        if is_arg_start(first) {
            let (arg, r) = Arg::parse(remains)?;
            remains = r;
            args.push(arg);
            while remains.starts_with(',') {
                remains = consume(remains, ',')?;
                let (arg, r) = Arg::parse(remains)?;
                remains = r;
                args.push(arg);
            }
        }

        let remains = consume(remains, ')')?;

        Ok((args, remains))
    }
}

impl Method {
    fn parse(method_def: &str) -> ParsingResult<(Method, &str)> {
        let (name, remains) = parse_ident(method_def)?;
        let (args, remains) = Arg::parse_args(remains)?;
        Ok((
            Method {
                name,
                args,
                raw: method_def[..method_def.len() - remains.len()].to_string(),
            },
            remains,
        ))
    }
}

impl MethodAndTypes {
    pub fn parse(method_def: &str) -> ParsingResult<Self> {
        let method_def = method_def;
        let mut parsed_types = HashMap::new();
        let mut type_sequences = vec![];
        let (method, mut types) = Method::parse(method_def)?;
        while !types.is_empty() {
            let (ty, remains) = Method::parse(types)?;
            type_sequences.push(ty.name.clone());
            parsed_types.insert(ty.name.clone(), ty);
            types = remains;
        }
        Ok(MethodAndTypes {
            method,
            types: parsed_types,
            type_sequences,
        })
    }
}

fn parse_ident(text: &str) -> ParsingResult<(String, &str)> {
    let mut chars = text.chars();
    if text.is_empty() || !is_arg_start(chars.next().unwrap()) {
        return Err(ParsingError::InvalidMetaTransactionMethodName);
    }

    let mut i = 1;
    for c in chars {
        if !is_arg_char(c) {
            break;
        }
        i += 1;
    }
    Ok((text[..i].to_string(), &text[i..]))
}

/// Tokenizer a type specifier from a method definition
/// E.g. text: "uint256[] petIds,..."
/// returns: "uint256[]", " petIds,..."
/// "uint256[]" is not parsed further to "an array of uint256" in this fn
fn parse_type_raw(text: &str) -> ParsingResult<(String, &str)> {
    let i = text
        .find(' ')
        .ok_or(ParsingError::InvalidMetaTransactionMethodName)?;
    Ok((text[..i].to_string(), &text[i..]))
}

/// Consume next char in text, it must be c or return parse error
/// return text without the first char
fn consume(text: &str, c: char) -> ParsingResult<&str> {
    let first = text.chars().next();
    if first.is_none() || first.unwrap() != c {
        return Err(ParsingError::InvalidMetaTransactionMethodName);
    }

    Ok(&text[1..])
}

/// Return true if c can be used as first char of a evm method arg
fn is_arg_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

/// Return true if c can be used as consequent char of a evm method arg
fn is_arg_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Return a signature of the method_def with additional args
/// E.g. methods_signature(Methods before parse: "adopt(uint256 petId,PetObj petobj)PetObj(string name)")
/// -> "adopt(uint256,PetObj)"
fn method_signature(method_and_type: &MethodAndTypes) -> String {
    let mut result = method_and_type.method.name.clone();
    result.push('(');
    for (i, arg) in method_and_type.method.args.iter().enumerate() {
        if i > 0 {
            result.push(',');
        }
        result.push_str(&arg.type_raw);
    }
    result.push(')');
    result
}

/// Decode rlp-encoded args into vector of Values
fn rlp_decode(args: &[u8]) -> ParsingResult<Vec<RlpValue>> {
    let rlp = Rlp::new(args);
    let res: core::result::Result<Vec<RlpValue>, DecoderError> = rlp.as_list();
    res.map_err(|_| ParsingError::InvalidMetaTransactionFunctionArg)
}

/// eip-712 hash a single argument, whose type is ty, and value is value.
/// Definition of all types is in `types`.
fn eip_712_hash_argument(
    ty: &ArgType,
    value: &RlpValue,
    types: &HashMap<String, Method>,
) -> ParsingResult<Vec<u8>> {
    match ty {
        ArgType::String | ArgType::Bytes => {
            eip_712_rlp_value(value, |b| Ok(keccak(b).as_bytes().to_vec()))
        }
        ArgType::Byte(_) => eip_712_rlp_value(value, |b| Ok(b.clone())),
        // TODO: ensure rlp int is encoded as sign extended uint256, otherwise this is wrong
        ArgType::Uint | ArgType::Int | ArgType::Bool => eip_712_rlp_value(value, |b| {
            Ok(u256_to_arr(&U256::from_big_endian(b)).to_vec())
        }),
        ArgType::Address => {
            eip_712_rlp_value(value, |b| Ok(encode_address(Address::from_slice(b))))
        }
        ArgType::Array { inner, .. } => eip_712_rlp_list(value, |l| {
            let mut r = vec![];
            for element in l {
                r.extend_from_slice(&eip_712_hash_argument(inner, element, types)?);
            }
            Ok(keccak(&r).as_bytes().to_vec())
        }),
        ArgType::Custom(type_name) => eip_712_rlp_list(value, |l| {
            let struct_type = types
                .get(type_name)
                .ok_or(ParsingError::InvalidMetaTransactionFunctionArg)?;
            // struct_type.raw is with struct type with argument names (a "method_def"), so it follows
            // EIP-712 typeHash.
            let mut r = keccak(struct_type.raw.as_bytes()).as_bytes().to_vec();
            for (i, element) in l.iter().enumerate() {
                r.extend_from_slice(&eip_712_hash_argument(
                    &struct_type.args[i].t,
                    element,
                    types,
                )?);
            }
            Ok(keccak(&r).as_bytes().to_vec())
        }),
    }
}

/// EIP-712 hash a RLP list. f must contain actual logic of EIP-712 encoding
/// This function serves as a guard to assert value is a List instead of Value
fn eip_712_rlp_list<F>(value: &RlpValue, f: F) -> ParsingResult<Vec<u8>>
where
    F: Fn(&Vec<RlpValue>) -> ParsingResult<Vec<u8>>,
{
    match value {
        RlpValue::Bytes(_) => Err(ParsingError::InvalidMetaTransactionFunctionArg),
        RlpValue::List(l) => f(l),
    }
}

/// EIP-712 hash a RLP value. f must contain actual logic of EIP-712 encoding
/// This function serves as a guard to assert value is a Value instead of List
fn eip_712_rlp_value<F>(value: &RlpValue, f: F) -> ParsingResult<Vec<u8>>
where
    F: Fn(&Vec<u8>) -> ParsingResult<Vec<u8>>,
{
    match value {
        RlpValue::List(_) => Err(ParsingError::InvalidMetaTransactionFunctionArg),
        RlpValue::Bytes(b) => f(b),
    }
}

fn eth_abi_encode_args(
    args_decoded: &[RlpValue],
    methods: &MethodAndTypes,
) -> ParsingResult<Vec<u8>> {
    let mut tokens = vec![];
    for (i, arg) in args_decoded.iter().enumerate() {
        tokens.push(arg_to_abi_token(&methods.method.args[i].t, arg, methods)?);
    }
    Ok(encode(&tokens))
}

fn arg_to_abi_token(
    ty: &ArgType,
    arg: &RlpValue,
    methods: &MethodAndTypes,
) -> ParsingResult<ABIToken> {
    match ty {
        ArgType::String | ArgType::Bytes => {
            value_to_abi_token(arg, |b| Ok(ABIToken::Bytes(b.clone())))
        }
        ArgType::Byte(_) => value_to_abi_token(arg, |b| Ok(ABIToken::FixedBytes(b.clone()))),
        ArgType::Uint | ArgType::Int | ArgType::Bool => {
            value_to_abi_token(arg, |b| Ok(ABIToken::Uint(U256::from_big_endian(b))))
        }
        ArgType::Address => {
            value_to_abi_token(arg, |b| Ok(ABIToken::Address(Address::from_slice(b))))
        }
        ArgType::Array {
            inner,
            length: None,
        } => list_to_abi_token(arg, |l| {
            let mut tokens = vec![];
            for arg in l {
                tokens.push(arg_to_abi_token(inner, arg, methods)?);
            }
            Ok(ABIToken::Array(tokens))
        }),
        ArgType::Array {
            inner,
            length: Some(_),
        } => list_to_abi_token(arg, |l| {
            let mut tokens = vec![];
            for arg in l {
                tokens.push(arg_to_abi_token(inner, arg, methods)?);
            }
            Ok(ABIToken::FixedArray(tokens))
        }),
        ArgType::Custom(type_name) => list_to_abi_token(arg, |l| {
            let struct_type = methods
                .types
                .get(type_name)
                .ok_or(ParsingError::InvalidMetaTransactionFunctionArg)?;
            let mut tokens = vec![];
            for (i, element) in l.iter().enumerate() {
                tokens.push(arg_to_abi_token(&struct_type.args[i].t, element, methods)?);
            }
            Ok(ABIToken::Tuple(tokens))
        }),
    }
}

fn value_to_abi_token<F>(value: &RlpValue, f: F) -> ParsingResult<ABIToken>
where
    F: Fn(&Vec<u8>) -> ParsingResult<ABIToken>,
{
    match value {
        RlpValue::List(_) => Err(ParsingError::InvalidMetaTransactionFunctionArg),
        RlpValue::Bytes(b) => f(b),
    }
}

fn list_to_abi_token<F>(value: &RlpValue, f: F) -> ParsingResult<ABIToken>
where
    F: Fn(&Vec<RlpValue>) -> ParsingResult<ABIToken>,
{
    match value {
        RlpValue::Bytes(_) => Err(ParsingError::InvalidMetaTransactionFunctionArg),
        RlpValue::List(l) => f(l),
    }
}

/// eip-712 hash struct of entire meta txn and abi-encode function args to evm input
pub fn prepare_meta_call_args(
    domain_separator: &RawU256,
    account_id: &[u8],
    method_def: String,
    input: &InternalMetaCallArgs,
) -> ParsingResult<(RawU256, Vec<u8>)> {
    let mut bytes = Vec::new();
    let method_arg_start = match method_def.find('(') {
        Some(index) => index,
        None => return Err(ParsingError::InvalidMetaTransactionMethodName),
    };
    let arguments = "Arguments".to_string() + &method_def[method_arg_start..];
    // Note: method_def is like "adopt(uint256 petId,PetObj petObj)PetObj(string name,address owner)",
    // MUST have no space after `,`. EIP-712 requires hashStruct start by packing the typeHash,
    // See "Rationale for typeHash" in https://eips.ethereum.org/EIPS/eip-712#definition-of-hashstruct
    // method_def is used here for typeHash
    let types = "NearTx(string evmId,uint256 nonce,uint256 feeAmount,address feeAddress,address contractAddress,uint256 value,string contractMethod,Arguments arguments)".to_string() + &arguments;
    bytes.extend_from_slice(keccak(types.as_bytes()).as_bytes());
    bytes.extend_from_slice(keccak(account_id).as_bytes());
    bytes.extend_from_slice(&u256_to_arr(&input.nonce));
    bytes.extend_from_slice(&input.fee_amount.to_bytes());
    bytes.extend_from_slice(&encode_address(input.fee_address));
    bytes.extend_from_slice(&encode_address(input.contract_address));
    bytes.extend_from_slice(&input.value.to_bytes());

    let methods = MethodAndTypes::parse(&method_def)?;
    let method_sig = method_signature(&methods);
    bytes.extend_from_slice(keccak(method_sig.as_bytes()).as_bytes());

    let mut arg_bytes = Vec::new();
    arg_bytes.extend_from_slice(keccak(arguments.as_bytes()).as_bytes());
    let args_decoded: Vec<RlpValue> = rlp_decode(&input.input)?;
    if methods.method.args.len() != args_decoded.len() {
        return Err(ParsingError::ArgsLengthMismatch);
    }
    for (i, arg) in args_decoded.iter().enumerate() {
        arg_bytes.extend_from_slice(&eip_712_hash_argument(
            &methods.method.args[i].t,
            arg,
            &methods.types,
        )?);
    }

    // ETH-ABI require function selector to use method_sig, instead of method_name,
    // See https://docs.soliditylang.org/en/v0.7.5/abi-spec.html#function-selector
    // Above spec is not completely clear, this implementation shows signature is the one without
    // argument name:
    // https://github.com/rust-ethereum/ethabi/blob/69285cf6b6202d9faa19c7d0239df6a2bd79d55f/ethabi/src/signature.rs#L28
    let method_selector = method_sig_to_abi(&method_sig);
    let args_eth_abi = eth_abi_encode_args(&args_decoded, &methods)?;
    let input = [method_selector.to_vec(), args_eth_abi.to_vec()].concat();

    let arg_bytes_hash: RawU256 = keccak(&arg_bytes).into();
    bytes.extend_from_slice(&arg_bytes_hash);

    let message: RawU256 = keccak(&bytes).into();
    let mut bytes = Vec::with_capacity(2 + 32 + 32);
    bytes.extend_from_slice(&[0x19, 0x01]);
    bytes.extend_from_slice(domain_separator);
    bytes.extend_from_slice(&message);
    Ok((keccak(&bytes).into(), input))
}

/// Parse encoded `MetaCallArgs`, validate with given domain and account and recover the sender's address from the signature.
/// Returns error if method definition or arguments are wrong, invalid signature or EC recovery failed.
pub fn parse_meta_call(
    domain_separator: &RawU256,
    account_id: &[u8],
    args: Vec<u8>,
) -> ParsingResult<InternalMetaCallArgs> {
    let meta_tx =
        MetaCallArgs::try_from_slice(&args).map_err(|_| ParsingError::ArgumentParseError)?;
    let nonce = U256::from(meta_tx.nonce);
    let fee_amount = Wei::new(U256::from(meta_tx.fee_amount));
    let fee_address = Address::from(meta_tx.fee_address);
    let contract_address = Address::from(meta_tx.contract_address);
    let value = Wei::new(U256::from(meta_tx.value));

    let mut result = InternalMetaCallArgs {
        sender: Address::zero(),
        nonce,
        fee_amount,
        fee_address,
        contract_address,
        value,
        input: meta_tx.args,
    };
    let (msg, input) =
        prepare_meta_call_args(domain_separator, account_id, meta_tx.method_def, &result)?;
    let mut signature: [u8; 65] = [0; 65];
    signature[64] = meta_tx.v;
    signature[..64].copy_from_slice(&meta_tx.signature);
    match crate::precompiles::ecrecover(H256::from_slice(&msg), &signature) {
        Ok(sender) => {
            result.sender = sender;
            result.input = input;
            Ok(result)
        }
        Err(_) => Err(ParsingError::InvalidEcRecoverSignature),
    }
}

#[cfg(test)]
mod tests {
    use super::ArgType;
    use rand::Rng;

    #[test]
    fn test_parse_type() {
        // # atomic types

        // ## bytesN
        for n in 1..=32 {
            let s = format!("bytes{}", n);
            assert_arg_type(&s, ArgType::Byte(n));
        }
        assert_arg_type("byte", ArgType::Byte(1));

        // ## uintN
        for n in 1..=32 {
            let s = format!("uint{}", 8 * n);
            assert_arg_type(&s, ArgType::Uint);
        }
        assert_arg_type("uint", ArgType::Uint);

        // ## intN
        for n in 1..=32 {
            let s = format!("int{}", 8 * n);
            assert_arg_type(&s, ArgType::Int);
        }
        assert_arg_type("int", ArgType::Int);

        // ## bool
        assert_arg_type("bool", ArgType::Bool);

        // ## address
        assert_arg_type("address", ArgType::Address);

        // ## custom
        let mut rng = rand::thread_rng();
        for _ in 0..u8::MAX {
            let name = rand_identifier(&mut rng);
            assert_arg_type(&name, ArgType::Custom(name.clone()));
        }

        // # dynamic types

        // ## bytes
        assert_arg_type("bytes", ArgType::Bytes);

        // ## string
        assert_arg_type("string", ArgType::String);

        // # arrays
        let inner_types: Vec<String> = (1..=32)
            .map(|n| format!("bytes{}", n))
            .chain((1..=32).map(|n| format!("uint{}", 8 * n)))
            .chain((1..=32).map(|n| format!("int{}", 8 * n)))
            .chain(std::iter::once("bool".to_string()))
            .chain(std::iter::once("address".to_string()))
            .chain(std::iter::once(rand_identifier(&mut rng)))
            .chain(std::iter::once("bytes".to_string()))
            .chain(std::iter::once("string".to_string()))
            .collect();
        for t in inner_types {
            let inner_type = super::parse_type(&t).ok().unwrap();
            let size: Option<u8> = rng.gen();

            // single array
            let single_array_string = create_array_type_string(&t, size);
            let expected = ArgType::Array {
                length: size.map(|x| x as u64),
                inner: Box::new(inner_type),
            };
            assert_arg_type(&single_array_string, expected.clone());

            // nested array
            let inner_type = expected;
            let size: Option<u8> = rng.gen();
            let nested_array_string = create_array_type_string(&single_array_string, size);
            let expected = ArgType::Array {
                length: size.map(|x| x as u64),
                inner: Box::new(inner_type),
            };
            assert_arg_type(&nested_array_string, expected);
        }

        // # errors
        // ## only numbers
        super::parse_type("27182818").unwrap_err();
        // ## invalid characters
        super::parse_type("Some.InvalidType").unwrap_err();
        super::parse_type("Some::NotType").unwrap_err();
        super::parse_type("*AThing*").unwrap_err();
    }

    fn create_array_type_string(inner_type: &str, size: Option<u8>) -> String {
        format!(
            "{}[{}]",
            inner_type,
            size.map(|x| x.to_string()).unwrap_or(String::new())
        )
    }

    fn assert_arg_type(s: &str, expected: ArgType) {
        assert_eq!(super::parse_type(s).ok().unwrap(), expected);
    }

    fn rand_identifier<T: Rng>(rng: &mut T) -> String {
        use rand::distributions::Alphanumeric;
        use rand::seq::IteratorRandom;

        // The first character must be a letter, so we sample that separately.
        let first_char = ('a'..='z').chain('A'..='Z').choose(rng).unwrap();
        let other_letters = (0..7).map(|_| char::from(rng.sample(Alphanumeric)));

        std::iter::once(first_char).chain(other_letters).collect()
    }
}
