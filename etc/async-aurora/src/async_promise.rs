use near_sdk::AccountId;

pub const ERR_INVALID_PROMISE: &str = "Invalid promise format";

pub enum CombinatorType {
    And,
    Then,
}

pub struct CombinatorDescription {
    pub promise_index: u8,
    pub combinator_type: CombinatorType,
}

pub struct PromiseDescription {
    pub target: AccountId,
    pub method_name: Vec<u8>,
    pub arguments: Vec<u8>,
    pub gas: u64,
    pub combinator: Option<CombinatorDescription>,
}

pub fn parse_promises(input: String) -> Vec<PromiseDescription> {
    let striped_input = input.strip_prefix("promises:").expect(ERR_INVALID_PROMISE);
    let iter_list = striped_input.split("##");
    let mut result = Vec::new();

    for item in iter_list {
        let mut iter = item.split('#');
        let target = iter.next().expect(ERR_INVALID_PROMISE).into();
        let method_name = iter.next().expect(ERR_INVALID_PROMISE).into();
        let arguments = iter.next().expect(ERR_INVALID_PROMISE).into();
        let gas = iter.next().expect(ERR_INVALID_PROMISE).parse::<u64>().expect(ERR_INVALID_PROMISE);
        let mut combinator: Option<CombinatorDescription> = None;

        if let Some(index_str) = iter.next() {
            let promise_index = index_str.parse::<u8>().expect(ERR_INVALID_PROMISE);
            let type_str = iter.next().expect(ERR_INVALID_PROMISE);
            let combinator_type = if type_str == "&" {CombinatorType::And} else {CombinatorType::Then};
            combinator = Some(CombinatorDescription {
                promise_index,
                combinator_type,
            });
        }

        let promise_description = PromiseDescription {
            target,
            method_name,
            arguments,
            gas,
            combinator: combinator,
        };
        result.push(promise_description);
    }

    result
}