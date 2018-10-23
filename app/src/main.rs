extern crate serde;
extern crate docopt;
#[macro_use]
extern crate serde_derive;
extern crate ethereum_types;
extern crate ethkey_sgx_app;

use docopt::Docopt;
use ethereum_types::Address;
use self::utils::{keyfile_exists, print_hex, get_affirmation};
use ethkey_sgx_app::{
    show_private_key, 
    generate_keypair, 
    destroy_keypair,
    get_eth_address, 
    get_public_key, 
    sign_message, 
    verify,
    utils
};

pub static DEFAULT_KEYPAIR_PATH: &'static str = "./encrypted_keypair.txt";

static USAGE: &'static str = "
Intel SGX Ethereum Key Management CLI.
    Copyright: 2018 Oraclize.it
    Questions: greg@oraclize.it

Usage:  ethkey_sgx                                              [-h | --help]
        ethkey_sgx generate                                     [--keyfile=<path>]
        ethkey_sgx show public                                  [--keyfile=<path>]
        ethkey_sgx show secret                                  [--keyfile=<path>]
        ethkey_sgx show address                                 [--keyfile=<path>] 
        ethkey_sgx sign msg <message>                           [--keyfile=<path>] [-n | --noprefix]
        ethkey_sgx verify <address> <message> <signature>       [--keyfile=<path>] [-n | --noprefix]
        ethkey_sgx destroy                                      [--keyfile=<path>]
        ethkey_sgx sign tx [--value=<Wei>] [--gaslimit=<uint>] [--keyfile=<path>] [--gasprice=<Wei>] [--nonce=<uint>] [--data=<string>]

Commands:
    generate            ❍ Generates an secp256k1 keypair inside an SGX enclave, encrypts
                        them & saves to disk as either ./encrypted_keypair.txt in the
                        current directory, or at the passed in path.
    show public         ❍ Log the public key from the given encrypted keypair to the console.
    show secret         ❍ Log the private key from the given encrypted keypair to the console.
    sign tx             ❍ Signs a transaction: WIP
    sign msg            ❍ Signs a passed in message using key pair provided, otherwise uses
                        default keypair if it exists. Defaults to using the ethereum message
                        prefix and ∴ signatures are ECRecoverable.
   verify               ❍ Verify a given address signed a given message with a given signature. 
   destroy              ❍ Destroys a given key file's monotonic counters, rendering the keyfile
                        unusable, before erasing the encrypted keyfile itself. Use with caution!

Options:
    -h, --help          ❍ Show this usage message.

    --keyfile=<path>    ❍ Path to desired encrypted keyfile. [default: ./encrypted_keypair]

    --value=<Wei>       ❍ Amount of ether to send with transaction in Wei [default: 0]

    --gaslimit=<uint>   ❍ Amount of gas to send with transaction [default: 210000]

    --gasprice=<Wei>    ❍ Gas price for transaction in Wei [default: 20000000000]

    --nonce=<uint>      ❍ Nonce of transaction in Wei [default: 0]

    --data=<string>     ❍ Additional data to send with transaction [default:  ]

    --value=<Wei>       ❍ Amount of ether to send with transaction in Wei [default: 0]

    -n, --noprefix      ❍ Does not add the ethereum message prefix when signing or verifying 
                        a signed message. Messages signed with no prefix are NOT ECRecoverable!
";

#[derive(Debug, Deserialize)]
struct Args {
    cmd_tx: bool,
    cmd_msg: bool,
    cmd_sign: bool,
    cmd_show: bool,
    flag_value: u64,
    flag_nonce: u64,
    cmd_public: bool,
    cmd_secret: bool,
    cmd_verify: bool,
    cmd_address: bool,
    cmd_destroy: bool,
    flag_data: String,
    cmd_generate: bool,
    flag_gasprice: u64,
    flag_gaslimit: u64,
    flag_noprefix: bool,
    arg_message: String,
    arg_address: String,
    flag_keyfile: String,
    arg_signature: String
}
/*
 * NOTE: Initial version of MC will be MRSIGNER not MRENCLAVE.
 * TODO: Use SGX time to log the last time key file was accessed. (This & above need bigger key struc!)
 * TODO: Store address in hex in keyfile!
 * TODO: Show full ethereum address!
 * TODO: Add option to verify via the hash too?
 * TODO: Use MRENCLAVE to tie a sealed thingy to this specific enclave!
 * TODO: Have a method to view the values of the mcs (should still increment the accesses obvs!)
 * */
fn main() {
    Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .map(execute)
        .unwrap_or_else(|e| e.exit());
}

fn execute(args: Args) -> () {
    match args {
        Args {cmd_show: true, ..}     => match_show(args),
        Args {cmd_sign: true, ..}     => match_sign(args), 
        Args {cmd_destroy: true, ..}  => destroy(args.flag_keyfile),
        Args {cmd_generate: true, ..} => generate(args.flag_keyfile),    
        Args {cmd_verify: true, ..}   => verify(&args.arg_address.parse().unwrap(), args.arg_message, args.arg_signature, args.flag_noprefix), // FIXME: Fix the unwrap! 
        _ => println!("{}", USAGE)
    }
}

fn match_show(args: Args) -> () {
    match args {
        Args {cmd_public: true, ..}  => show_pub(args.flag_keyfile),
        Args {cmd_secret: true, ..}  => show_priv(args.flag_keyfile),
        Args {cmd_address: true, ..} => show_addr(args.flag_keyfile),
        _ => println!("{}", USAGE)
    }
}

fn match_sign(args: Args) -> () {
    match args {
        Args {cmd_msg: true, ..} => sign(args.flag_keyfile, args.arg_message, args.flag_noprefix),
        Args {cmd_tx: true, ..}  => println!("Signing transaction now sir :P"),
        _ => println!("{}", USAGE)
    }
}

fn generate(path: String) -> () {
    match keyfile_exists(&path) {
        false => create_keypair(&path),
        true  => {
            println!("[!] WARNING! Something already exists at {} and will be overwritten.\n", &path); 
            match get_affirmation("This cannot be undone!".to_string()) {
                false => println!("[-] Affirmation not received, exiting."),
                true  => create_keypair(&path)
            }
        }
    }
}

fn destroy(path: String) -> () {
    match keyfile_exists(&path) {
        false => println!("[-] Cannot destroy key, no keyfile found at {}", &path),
        true  => {
            println!("[!] WARNING! Key file at {} will be destroyed.\n", &path); 
            match get_affirmation("This cannot be undone!".to_string()) {
                false => println!("[-] Affirmation not received, exiting."),
                true  => match destroy_keypair::run(&path) {
                    Ok(_)  => (),
                    Err(e) => println!("[-] Error destroying key pair: {}", e)
                }
            }
        }
    }
}

fn show_priv(path: String) -> () {
    match get_affirmation("You are about to log your private key to the console!".to_string()) {
        false => println!("[-] Affirmation not received, exiting."),
        true  => {
            match show_private_key::run(&path) {
                Ok(_)  => (),
                Err(e) => println!("[-] Error retreiving plaintext private key from {}:\n\t{:?}", &path, e)
            }
        }
    }
}

fn create_keypair(path: &String) -> (){
    match generate_keypair::run(&path) {
        Ok(_)  => println!("[+] Key pair successfully saved to {}!", path),
        Err(e) => println!("[-] Error generating keypair:\n\t{:?}", e)
    };
}

fn sign(path: String, message: String, no_prefix: bool) -> () {
    match sign_message::run(&path, message, no_prefix) {
        Err(e) => println!("[-] Error signing message with key from {}:\n\t{:?}", &path, e),
        Ok(k)  => {
            match no_prefix { // TODO: Print better
                true  => {println!("[+] Message signature (no prefix): ");print_hex(k.to_vec())},
                false => {println!("[+] Message signature (with prefix): ");print_hex(k.to_vec())}
            }
        }
    }
}

fn show_pub(path: String) -> () {
    match get_public_key::run(&path) {
        Ok(k)  => println!("[+] {:?}", k),
        Err(e) => println!("[-] Error retreiving plaintext public key from {}:\n\t{:?}", &path, e)
    }
}

fn show_addr(path: String) -> () {
    match get_eth_address::run(&path) {
        Ok(a)  => println!("[+] Ethereum Address: {}", a),
        Err(e) => println!("[-] Error retreiving Ethereum Address from: {}:\n\t{:?}", &path, e)
    }
}

fn verify(address: &Address, message: String, signature: String, no_prefix: bool) -> () {
    match verify::run(address, message, signature, no_prefix) {
        Err(e) => println!("[-] Error verifying signature: {}", e),
        Ok(b)  => {
            match b {
                true  => println!("[+] Signature verified! Message was signed with Ethereum Address: {}", address),
                false => println!("[!] Signature verification failed. Message was NOT signed with Ethereum Address: {}", address)
            }
        }
    }
}
