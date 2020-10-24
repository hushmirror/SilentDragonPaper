use std::thread;
use hex;
use base58::{ToBase58};
use bech32::{Bech32, u5, ToBase32};
use rand::{Rng, ChaChaRng, FromEntropy, SeedableRng};
use json::{array, object};
use sha2::{Sha256, Digest};
use std::io;
use std::io::Write;
use std::sync::mpsc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::panic;
use std::time::{SystemTime};
use zcash_primitives::zip32::{DiversifierIndex, DiversifierKey, ChildIndex, ExtendedSpendingKey, ExtendedFullViewingKey};

/// A trait for converting a [u8] to base58 encoded string.
pub trait ToBase58Check {
    /// Converts a value of `self` to a base58 value, returning the owned string.
    /// The version is a coin-specific prefix that is added. 
    /// The suffix is any bytes that we want to add at the end (like the "iscompressed" flag for 
    /// Secret key encoding)
    fn to_base58check(&self, version: &[u8], suffix: &[u8]) -> String;
}

impl ToBase58Check for [u8] {
    fn to_base58check(&self, version: &[u8], suffix: &[u8]) -> String {
        let mut payload: Vec<u8> = Vec::new();
        payload.extend_from_slice(version);
        payload.extend_from_slice(self);
        payload.extend_from_slice(suffix);
        
        let checksum = double_sha256(&payload);
        payload.append(&mut checksum[..4].to_vec());
        payload.to_base58()
    }
}

/// Sha256(Sha256(value))
pub fn double_sha256(payload: &[u8]) -> Vec<u8> {
    let h1 = Sha256::digest(&payload);
    let h2 = Sha256::digest(&h1);
    h2.to_vec()
}

/// Parameters used to generate addresses and private keys. Look in chainparams.cpp (in zcashd/src)
/// to get these values. 
/// Usually these will be different for testnet and for mainnet.
pub struct CoinParams {
    pub taddress_version: [u8; 2],
    pub tsecret_prefix  : [u8; 1],
    pub zaddress_prefix : String,
    pub zsecret_prefix  : String,
    pub zviewkey_prefix : String,
    pub cointype        : u32,
}

pub fn params() -> CoinParams {
        CoinParams {
            taddress_version : [0x1C, 0xB8],
            tsecret_prefix   : [0xBC],
            zaddress_prefix  : "zs".to_string(),
            zsecret_prefix   : "secret-extended-key-main".to_string(),
            zviewkey_prefix  : "zviews".to_string(),
            cointype         : 133
        }
}

pub fn increment(s: &mut [u8; 32]) -> Result<(), ()> {
    for k in 0..32 {
        s[k] = s[k].wrapping_add(1);
        if s[k] != 0 {
            // No overflow
            return Ok(());
        }
    }
    // Overflow
    Err(())
}

// Turn the prefix into Vec<u5>, so it can be matched directly without any encoding overhead.
fn get_bech32_for_prefix(prefix: String) -> Result<Vec<u5>, String> {
    // Reverse character set. Maps ASCII byte -> CHARSET index on [0,31]
    const CHARSET_REV: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        15, -1, 10, 17, 21, 20, 26, 30,  7,  5, -1, -1, -1, -1, -1, -1,
        -1, 29, -1, 24, 13, 25,  9,  8, 23, -1, 18, 22, 31, 27, 19, -1,
        1,  0,  3, 16, 11, 28, 12, 14,  6,  4,  2, -1, -1, -1, -1, -1,
        -1, 29, -1, 24, 13, 25,  9,  8, 23, -1, 18, 22, 31, 27, 19, -1,
        1,  0,  3, 16, 11, 28, 12, 14,  6,  4,  2, -1, -1, -1, -1, -1
    ];

    let mut ans = Vec::new();
    for c in prefix.chars() {
        if CHARSET_REV[c as usize] == -1 {
            return Err(format!("Invalid character in prefix: '{}'", c));
        }
        ans.push(u5::try_from_u8(CHARSET_REV[c as usize] as u8).expect("Should be able to convert to u5"));
    }

    return Ok(ans);
}

fn encode_address(spk: &ExtendedSpendingKey) -> String {
    let (_d, addr) = spk.default_address().expect("Cannot get result");

    // Address is encoded as a bech32 string
    let mut v = vec![0; 43];

    v.get_mut(..11).unwrap().copy_from_slice(&addr.diversifier.0);
    addr.pk_d.write(v.get_mut(11..).unwrap()).expect("Cannot write!");
    let checked_data: Vec<u5> = v.to_base32();
    let encoded : String = Bech32::new(params().zaddress_prefix.into(), checked_data).expect("bech32 failed").to_string();
    
    return encoded;
}

fn encode_privatekey(spk: &ExtendedSpendingKey) -> String {
    // Private Key is encoded as bech32 string
    let mut vp = Vec::new();
    spk.write(&mut vp).expect("Can't write private key");
    let c_d: Vec<u5> = vp.to_base32();
    let encoded_pk = Bech32::new(params().zsecret_prefix.into(), c_d).expect("bech32 failed").to_string();

    return encoded_pk;
}

/// A single thread that grinds through the Diversifiers to find the defualt key that matches the prefix
pub fn vanity_thread(entropy: &[u8], prefix: String, tx: mpsc::Sender<String>, please_stop: Arc<AtomicBool>) {
    
    let mut seed: [u8; 32] = [0; 32];
    seed.copy_from_slice(&entropy[0..32]);

    let di = DiversifierIndex::new();
    let vanity_bytes = get_bech32_for_prefix(prefix).expect("Bad char in prefix");

    let master_spk = ExtendedSpendingKey::from_path(&ExtendedSpendingKey::master(&seed),
                            &[ChildIndex::Hardened(32), ChildIndex::Hardened(params().cointype), ChildIndex::Hardened(0)]);

    let mut spkv = vec![];
    master_spk.write(&mut spkv).unwrap();

    let mut i: u32 = 0;
    loop {
        if increment(&mut seed).is_err() {
            return;
        }

        let dk = DiversifierKey::master(&seed);
        let (_ndk, nd) = dk.diversifier(di).unwrap();

        // test for nd
        let mut isequal = true;
        for i in 0..vanity_bytes.len() {
            if vanity_bytes[i] != nd.0.to_base32()[i] {
                isequal = false;
                break;
            }
        }

        if isequal { 
            let len = spkv.len();
            spkv[(len-32)..len].copy_from_slice(&dk.0[0..32]);
            let spk = ExtendedSpendingKey::read(&spkv[..]).unwrap();

            
            let encoded = encode_address(&spk);
            let encoded_pk = encode_privatekey(&spk);
            
            let wallet = array!{object!{
                "num"           => 0,
                "address"       => encoded,
                "private_key"   => encoded_pk,
                "type"          => "zaddr"}};
            
            tx.send(json::stringify_pretty(wallet, 2)).unwrap();
            return;
        }

        i = i + 1;
        if i%5000 == 0 {
            if please_stop.load(Ordering::Relaxed) {
                return;
            }
            tx.send("Processed:5000".to_string()).unwrap();
        }

        if i == 0 { return; }
    }
}

fn pretty_duration(secs: f64) -> (String, String) {
    let mut expected_dur  = "sec";
    let mut expected_time = secs;

    if expected_time > 60.0 {
        expected_time /= 60.0;
        expected_dur = "min";
    }
    if expected_time > 60.0 {
        expected_time /= 60.0;
        expected_dur = "hours";
    }
    if expected_time > 24.0 {
        expected_time /= 24.0;
        expected_dur = "days";
    }
    if expected_time > 30.0 {
        expected_time /= 30.0;
        expected_dur = "months";
    }
    if expected_time > 12.0 {
        expected_time /= 12.0;
        expected_dur = "years";
    }

    return (format!("{:.*}", 0, expected_time), expected_dur.to_string());
}

/// Generate a vanity address with the given prefix.
pub fn generate_vanity_wallet(num_threads: u32, prefix: String) -> Result<String, String> {
    // Test the prefix first
    match get_bech32_for_prefix(prefix.clone()) {
        Ok(_)  => (),
        Err(e) => return Err(format!("{}. Note that ['b', 'i', 'o', '1'] are not allowed in addresses.", e))
    };

    // Get 32 bytes of system entropy
    let mut system_rng = ChaChaRng::from_entropy();    
    
    let (tx, rx) = mpsc::channel();
    let please_stop = Arc::new(AtomicBool::new(false));

    let mut handles = Vec::new();

    for _i in 0..num_threads {
        //let testnet_local = is_testnet.clone();
        let prefix_local = prefix.clone();
        let tx_local = mpsc::Sender::clone(&tx);
        let ps_local = please_stop.clone();
    
        let mut entropy: [u8; 32] = [0; 32];
        system_rng.fill(&mut entropy);
    
        let handle = thread::spawn(move || {
            vanity_thread(&entropy, prefix_local, tx_local, ps_local);
        });
        handles.push(handle);
    }
    
    let mut processed: u64   = 0;
    let now = SystemTime::now();

    let wallet: String;

    // Calculate the estimated time
    let expected_combinations = (32 as f64).powf(prefix.len() as f64);

    loop {
        let recv = rx.recv().unwrap();
        if recv.starts_with(&"Processed") {
            processed = processed + 5000;
            let timeelapsed = now.elapsed().unwrap().as_secs() + 1; // Add one second to prevent any divide by zero problems.

            let rate = processed / timeelapsed;            
            let expected_secs = expected_combinations / (rate as f64);

            let (s, d) = pretty_duration(expected_secs);

            print!("Checking addresses at {}/sec on {} CPU threads. [50% ETA = {} {}]   \r", rate, num_threads, s, d);
            io::stdout().flush().ok().unwrap();
        } else {
            // Found a solution
            println!("");   // To clear the previous inline output to stdout;
            wallet = recv;

            please_stop.store(true, Ordering::Relaxed);
            break;
        } 
    }

    for handle in handles {
        handle.join().unwrap();
    }    

    return Ok(wallet);
}

/// Generate a series of `count` addresses and private keys. 
pub fn generate_wallet(nohd: bool, zcount: u32, tcount: u32, user_entropy: &[u8]) -> String {        
    // Get 32 bytes of system entropy
    let mut system_entropy:[u8; 32] = [0; 32]; 
    {
        let result = panic::catch_unwind(|| {
            ChaChaRng::from_entropy()
        });

        let mut system_rng = match result {
            Ok(rng)     => rng,
            Err(_e)     => ChaChaRng::from_seed([0; 32])
        };

        system_rng.fill(&mut system_entropy);
    }    
    
    // Add in user entropy to the system entropy, and produce a 32 byte hash... 
    let mut state = sha2::Sha256::new();
    state.input(&system_entropy);
    state.input(&user_entropy);
    
    let mut final_entropy: [u8; 32] = [0; 32];
    final_entropy.clone_from_slice(&double_sha256(&state.result()[..]));

    // ...which will we use to seed the RNG
    let mut rng = ChaChaRng::from_seed(final_entropy);

    if !nohd {
        // Allow HD addresses, so use only 1 seed        
        let mut seed: [u8; 32] = [0; 32];
        rng.fill(&mut seed);
        
        return gen_addresses_with_seed_as_json(zcount, tcount, |i| (seed.to_vec(), i));
    } else {
        // Not using HD addresses, so derive a new seed every time    
        return gen_addresses_with_seed_as_json(zcount, tcount, |_| {            
            let mut seed:[u8; 32] = [0; 32]; 
            rng.fill(&mut seed);
            
            return (seed.to_vec(), 0);
        });
    }    
}

/// Generate `count` addresses with the given seed. The addresses are derived from m/32'/cointype'/index' where 
/// index is 0..count
/// 
/// Note that cointype is 1 for testnet and 133 for mainnet
/// 
/// get_seed is a closure that will take the address number being derived, and return a tuple cointaining the 
/// seed and child number to use to derive this wallet. 
/// It is useful if we want to reuse (or not) the seed across multiple wallets.
fn gen_addresses_with_seed_as_json<F>(zcount: u32, tcount: u32, mut get_seed: F) -> String 
    where F: FnMut(u32) -> (Vec<u8>, u32)
{
    let mut ans = array![];

    // Note that for t-addresses, we don't use HD addresses
    let (seed, _) = get_seed(0);
    let mut rng_seed: [u8; 32] = [0; 32];
    rng_seed.clone_from_slice(&seed[0..32]);
    
    // First generate the Z addresses
    for i in 0..zcount {
        let (seed, child) = get_seed(i);
        let (addr, pk, _vk, path) = get_zaddress(&seed, child);
        ans.push(object!{
                "num"           => i,
                "address"       => addr,
                "private_key"   => pk,
                "type"          => "zaddr",
                "seed"          => path
        }).unwrap(); 
    }      

    // Next generate the T addresses
    // derive a RNG from the seed
    let mut rng = ChaChaRng::from_seed(rng_seed);

    for i in 0..tcount {        
        let (addr, pk_wif) = get_taddress(&mut rng);

        ans.push(object!{
            "num"               => i,
            "address"           => addr,
            "private_key"       => pk_wif,
            "type"              => "taddr"
        }).unwrap();
    }

    return json::stringify_pretty(ans, 2);
}

/// Generate a t address
fn get_taddress(rng: &mut ChaChaRng) -> (String, String) {
//    use secp256k1;
    use ripemd160::{Ripemd160};

    let mut sk_bytes: [u8; 32] = [0;32];

    // There's a small chance the generated private key bytes are invalid, so
    // we loop till we find bytes that are
    let sk = loop {    
        rng.fill(&mut sk_bytes);

        match secp256k1::SecretKey::parse(&sk_bytes) {
            Ok(s)  => break s,
            Err(_) => continue
        }
    };
    
    let pubkey = secp256k1::PublicKey::from_secret_key(&sk);

    // Address 
    let mut hash160 = Ripemd160::new();
    hash160.input(sha2::Sha256::digest(&pubkey.serialize_compressed().to_vec()));
    let addr = hash160.result().to_base58check(&params().taddress_version, &[]);

    // Private Key
    let pk_wif = sk_bytes.to_base58check(&params().tsecret_prefix, &[0x01]);  

    return (addr, pk_wif);
}

/// Generate a standard ZIP-32 address from the given seed at 32'/44'/0'/index
fn get_zaddress(seed: &[u8], index: u32) -> (String, String, String, json::JsonValue) {
   let spk: ExtendedSpendingKey = ExtendedSpendingKey::from_path(
            &ExtendedSpendingKey::master(seed),
            &[
                ChildIndex::Hardened(32),
                ChildIndex::Hardened(params().cointype),
                ChildIndex::Hardened(index)
            ],
        );
    let path = object!{
        "HDSeed"    => hex::encode(seed),
        "path"      => format!("m/32'/{}'/{}'", params().cointype, index)
    };

    let encoded = encode_address(&spk);
    let encoded_pk = encode_privatekey(&spk);

    // Viewing Key is encoded as bech32 string
    let mut vv = Vec::new();
    ExtendedFullViewingKey::from(&spk).write(&mut vv).expect("Can't write viewing key");
    let c_v: Vec<u5> = vv.to_base32();
    let encoded_vk = Bech32::new(params().zviewkey_prefix.into(), c_v).expect("bech32 failed").to_string();

    return (encoded, encoded_pk, encoded_vk, path);
}