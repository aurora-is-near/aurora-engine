//! This module defines data structure to produce traces compatible with geths "callTracer":
//! `https://github.com/ethereum/go-ethereum/blob/ad15050c7fbedd0f05a49e81400de18c2cc2c284/eth/tracers/native/call.go`

use aurora_engine_types::{types::Address, U256};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CallFrame {
    pub call_type: CallType,
    pub from: Address,
    pub to: Option<Address>,
    pub value: U256,
    pub gas: u64,
    pub gas_used: u64,
    pub input: Vec<u8>,
    pub output: Vec<u8>,
    pub error: Option<String>,
    pub calls: Vec<CallFrame>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CallTracer {
    pub call_stack: Vec<CallFrame>,
    pub top_level_transact: Option<CallFrame>,
}

impl CallTracer {
    fn end(&mut self, output: Vec<u8>, error: Option<&aurora_evm::ExitReason>) {
        if self.call_stack.is_empty() {
            debug_assert!(
                error.is_some(),
                "Empty stack can only occur if there is an early error"
            );
            debug_assert!(
                self.top_level_transact.is_some(),
                "Top level transact events always come before any exit event"
            );
            let frame = self.top_level_transact.take().unwrap_or_else(|| CallFrame {
                call_type: CallType::Call,
                from: Address::default(),
                to: None,
                value: U256::zero(),
                gas: 0,
                gas_used: 0,
                input: Vec::new(),
                output: Vec::new(),
                error: Some("Tracing bug: Exit before Enter".into()),
                calls: Vec::new(),
            });
            self.call_stack.push(frame);
        }

        // unwrap is safe because we push a new frame if the
        // stack was empty at the start of this method.
        let frame = self.call_stack.first_mut().unwrap();
        match error {
            None => {
                match frame.call_type {
                    // In CREATE case, output is set by the `CreateOutput` rather than the `Exit` event.
                    CallType::Create | CallType::Create2 => (),
                    _ => frame.output = output,
                }
            }
            Some(error) => {
                let error_message = format!("{error:?}");
                if error_message.to_lowercase().contains("revert") {
                    frame.output = output;
                }
                frame.error = Some(error_message);
            }
        }
    }

    fn enter(
        &mut self,
        call_type: CallType,
        from: Address,
        to: Address,
        input: Vec<u8>,
        gas: u64,
        value: U256,
    ) {
        let frame = CallFrame {
            call_type,
            from,
            to: Some(to),
            value,
            gas,
            gas_used: 0,
            input,
            output: Vec::new(),
            error: None,
            calls: Vec::new(),
        };
        self.call_stack.push(frame);
    }

    fn exit(&mut self, output: Vec<u8>, error: Option<&aurora_evm::ExitReason>) {
        if self.call_stack.len() <= 1 {
            return self.end(output, error);
        }

        let mut frame = self.call_stack.pop().unwrap();
        match error {
            None => {
                match frame.call_type {
                    // In CREATE case, output is set by the `CreateOutput` rather than the `Exit` event.
                    CallType::Create | CallType::Create2 => (),
                    _ => frame.output = output,
                }
            }
            Some(error) => {
                frame.error = Some(format!("{error:?}"));
                frame.output = output;
            }
        }

        self.call_stack.last_mut().unwrap().calls.push(frame);
    }

    fn update_gas_from_snapshot(&mut self, snapshot: Option<aurora_evm::gasometer::Snapshot>) {
        if let Some(snapshot) = snapshot {
            if let Some(frame) = self.call_stack.last_mut() {
                frame.gas = snapshot.gas_limit;
                frame.gas_used = snapshot.used_gas + snapshot.memory_gas;
            }
        }
    }

    #[cfg(feature = "serde")]
    #[must_use]
    pub fn serializable(mut self) -> Option<SerializableCallFrame> {
        if self.call_stack.len() != 1 {
            // If there is more than one element in `call_stack` then it must mean the trace did not complete
            // because there is only 1 top-level call. Note: additional frames are added as new scopes are entered,
            // but then the frames are coalesced as those scopes are existed.
            return None;
        }

        Some(self.call_stack.pop().unwrap().into())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallType {
    Call,
    StaticCall,
    DelegateCall,
    CallCode,
    Create,
    Create2,
    SelfDestruct,
}

impl AsRef<str> for CallType {
    fn as_ref(&self) -> &str {
        match self {
            Self::Call => "CALL",
            Self::StaticCall => "STATICCALL",
            Self::DelegateCall => "DELEGATECALL",
            Self::CallCode => "CALLCODE",
            Self::Create => "CREATE",
            Self::Create2 => "CREATE2",
            Self::SelfDestruct => "SELFDESTRUCT",
        }
    }
}

impl aurora_evm::gasometer::tracing::EventListener for CallTracer {
    fn event(&mut self, event: aurora_evm::gasometer::tracing::Event) {
        match event {
            // RecordRefund always comes at the end of an internal transaction and has all the gas information we need.
            aurora_evm::gasometer::tracing::Event::RecordRefund {
                refund: _,
                snapshot,
            } => self.update_gas_from_snapshot(snapshot),

            // Not useful
            aurora_evm::gasometer::tracing::Event::RecordCost { .. }
            | aurora_evm::gasometer::tracing::Event::RecordDynamicCost { .. }
            | aurora_evm::gasometer::tracing::Event::RecordStipend { .. }
            | aurora_evm::gasometer::tracing::Event::RecordTransaction { .. } => (),
        }
    }
}

impl aurora_evm::runtime::tracing::EventListener for CallTracer {
    fn event(&mut self, _event: aurora_evm::runtime::tracing::Event) {}
}

impl aurora_evm::tracing::EventListener for CallTracer {
    #[allow(clippy::too_many_lines)]
    fn event(&mut self, event: aurora_evm::tracing::Event) {
        match event {
            aurora_evm::tracing::Event::Call {
                code_address,
                transfer,
                input,
                target_gas,
                is_static,
                context,
            } => {
                let call_type = if is_static {
                    CallType::StaticCall
                } else if code_address == context.address {
                    CallType::Call
                } else if transfer.is_none() {
                    CallType::DelegateCall
                } else {
                    CallType::CallCode
                };

                self.enter(
                    call_type,
                    Address::new(context.caller),
                    Address::new(context.address),
                    input.to_vec(),
                    target_gas.unwrap_or_default(),
                    context.apparent_value,
                );
            }
            aurora_evm::tracing::Event::Create {
                caller,
                address,
                scheme,
                value,
                init_code,
                target_gas,
            } => {
                let call_type = match scheme {
                    aurora_evm::CreateScheme::Create2 { .. } => CallType::Create2,
                    aurora_evm::CreateScheme::Legacy { .. }
                    | aurora_evm::CreateScheme::Fixed(_) => CallType::Create, // Is Fixed even possible in production? With high probability not.
                };

                self.enter(
                    call_type,
                    Address::new(caller),
                    Address::new(address),
                    init_code.to_vec(),
                    target_gas.unwrap_or_default(),
                    value,
                );
            }
            aurora_evm::tracing::Event::CreateOutput { address, code } => {
                // `to` field should have been set to the address of the contract being created
                debug_assert_eq!(
                    Some(address),
                    self.call_stack
                        .last()
                        .and_then(|call_frame| call_frame.to.as_ref().map(Address::raw))
                );
                let current_frame = self.call_stack.last_mut();
                if let Some(frame) = current_frame {
                    frame.output = code.to_vec();
                }
            }
            aurora_evm::tracing::Event::Suicide {
                address,
                target,
                balance,
            } => {
                // TODO: gas = 0 is right?
                self.enter(
                    CallType::SelfDestruct,
                    Address::new(address),
                    Address::new(target),
                    Vec::new(),
                    0,
                    balance,
                );
                self.exit(Vec::new(), None);
            }
            // Exit event always comes after RecordRefund, so we don't need to worry about gas here (it's handled in RecordRefund)
            aurora_evm::tracing::Event::Exit {
                reason,
                return_value,
            } => {
                let error = match reason {
                    aurora_evm::ExitReason::Succeed(_) => None,
                    other => Some(other),
                };
                self.exit(return_value.to_vec(), error);
            }

            aurora_evm::tracing::Event::TransactCall {
                caller,
                address,
                value,
                data,
                gas_limit,
            } => {
                let frame = CallFrame {
                    call_type: CallType::Call,
                    from: Address::new(caller),
                    to: Some(Address::new(address)),
                    value,
                    gas: gas_limit,
                    gas_used: 0,
                    input: data.to_vec(),
                    output: Vec::new(),
                    error: None,
                    calls: Vec::new(),
                };
                self.top_level_transact = Some(frame);
            }

            aurora_evm::tracing::Event::TransactCreate {
                caller,
                value,
                init_code,
                gas_limit,
                address,
            } => {
                let frame = CallFrame {
                    call_type: CallType::Create,
                    from: Address::new(caller),
                    to: Some(Address::new(address)),
                    value,
                    gas: gas_limit,
                    gas_used: 0,
                    input: init_code.to_vec(),
                    output: Vec::new(),
                    error: None,
                    calls: Vec::new(),
                };
                self.top_level_transact = Some(frame);
            }

            aurora_evm::tracing::Event::TransactCreate2 {
                caller,
                value,
                init_code,
                gas_limit,
                address,
                ..
            } => {
                let frame = CallFrame {
                    call_type: CallType::Create2,
                    from: Address::new(caller),
                    to: Some(Address::new(address)),
                    value,
                    gas: gas_limit,
                    gas_used: 0,
                    input: init_code.to_vec(),
                    output: Vec::new(),
                    error: None,
                    calls: Vec::new(),
                };
                self.top_level_transact = Some(frame);
            }

            // not useful
            aurora_evm::tracing::Event::PrecompileSubcall { .. } => (),
        }
    }
}

#[cfg(feature = "serde")]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SerializableCallFrame {
    #[serde(rename = "type")]
    call_type: String,
    from: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    gas: String,
    #[serde(rename = "gasUsed")]
    gas_used: String,
    input: String,
    output: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    calls: Vec<SerializableCallFrame>,
}

#[cfg(feature = "serde")]
impl From<CallFrame> for SerializableCallFrame {
    fn from(frame: CallFrame) -> Self {
        let value = if frame.value.is_zero() {
            None
        } else {
            let value = frame.value;
            Some(format!("0x{value:x}"))
        };

        let gas = frame.gas;
        let gas_used = frame.gas_used;
        Self {
            call_type: frame.call_type.as_ref().into(),
            from: format!("0x{}", frame.from.encode()),
            to: frame.to.map(|addr| format!("0x{}", addr.encode())),
            value,
            gas: format!("0x{gas:x}"),
            gas_used: format!("0x{gas_used:x}"),
            input: format!("0x{}", hex::encode(&frame.input)),
            output: format!("0x{}", hex::encode(&frame.output)),
            error: frame.error,
            calls: frame.calls.into_iter().map(Into::into).collect(),
        }
    }
}
