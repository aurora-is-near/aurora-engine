const fs = require('fs');
const path = require('path');
const artifact = require(process.argv[2]);
if (!fs.existsSync('res')) {
    fs.mkdirSync('res');
}
const contractName = artifact.contractName;
fs.writeFileSync(path.join('res', contractName + '.hex'), artifact.bytecode);
fs.writeFileSync(path.join('res', contractName + '.bin'), Buffer.from(artifact.bytecode.substring(2), 'hex'));
