## hushPaperWallet Web
The paper wallet generator [is hosted at paper.hushwallet.co](https://paper.myhush.org).

This is a web version of the Hush Paper Wallet generator. It's mainly for illustrative purposes. If you want to generate a serious offline paper wallet, you should run [hushpaperwallet](https://github.com/MyHush/hushpaperwallet) offline. 

## Installing wasm-pack
You can run the web wallet locally. You need to install the following first:
1. [Rust 1.32+](https://www.rust-lang.org/tools/install)
2. [nodejs / npm](https://www.npmjs.com/get-npm)
3. wasm-pack
```
cargo install wasm-pack
```

### Running locally
You can run the web wallet locally.

```
cd hushpaperwallet/web
wasm-pack build
cd www
npm install
npm run start
```

This will start a local web server at `localhost:8080` where you can access the paper wallet
