EVM_ACCOUNT  = ENV['NEAR_EVM_ACCOUNT'] || 'evm.test.near'
RPC_ENDPOINT = ENV['NEAR_URL'] || 'http://localhost:3030'
EVM_VERSION  = File.read('VERSION').chomp

task default: %w(dump)

desc "Dump EVM contract state"
task :dump do |t|
   require 'base64'
   result = rpc_call(:query, request_type: 'view_state', account_id: EVM_ACCOUNT, prefix_base64: '', finality: 'final')
   evm_state = {}
   result['values'].each do |record|
     record_key, record_val = Base64.decode64(record['key']), Base64.decode64(record['value']).freeze
     record_type, evm_address, sstore_key = decode_evm_key(record_key)
     evm_account = evm_state[evm_address] ||= EVMAccount.new(evm_address)
     case record_type
       when :storage then evm_account.storage[sstore_key] = record_val
       else evm_account.send(:"#{record_type}=", record_val)
     end
   end
   evm_state.keys.sort.each do |evm_address|
     evm_account = evm_state[evm_address]
     puts "%s nonce=%s balance=%s code=%dB" % [hex(evm_address), u256(evm_account.nonce), u256(evm_account.balance), (evm_account.code || '').size]
     evm_account.storage.keys.sort.each do |sstore_key|
       sstore_val = evm_account.storage[sstore_key]
       puts "  %s %s" % [hex(sstore_key), hex(sstore_val)]
     end
   end
end

EVMAccount = Struct.new(:address, :nonce, :balance, :code, :storage) do
  def initialize(address)
    super(address, 0, 0, nil, {})
  end
end

def hex(bytes)
  '0x' << bytes.each_byte.map { |b| sprintf('%02x', b) }.join
end

def u256(bytes)
  bytes = bytes.gsub(/^\0+/, '')
  hex(bytes.empty? ? "\0" : bytes)
end

def decode_evm_key(key)
  case key[0].ord
    when 0 then [:code, key[1..]]
    when 1 then [:balance, key[1..]]
    when 2 then [:nonce, key[1..]]
    when 3 then [:storage, key[1..20], key[21..]]
  end
end

def rpc_call(method, **args)
  require 'json'
  require 'net/http'
  request = {
    jsonrpc: '2.0',
    id: 1,
    method: method.to_s,
    params: (args || {}).to_h.transform_keys(&:to_s),
  }
  headers = {'Content-Type' => 'application/json'}
  response = Net::HTTP.post(URI(RPC_ENDPOINT), request.to_json, headers)
  response = JSON.parse(response.body)
  response['result']
end
