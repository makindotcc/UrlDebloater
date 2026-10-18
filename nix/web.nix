{
  lib,
  stdenv,
  rustPlatform,
  zola,
  cacert,
  lld,
  wasmBindgenCli,
  src,
  cargoLock,
  mixerUrl ? "http://127.0.0.1:7777/",
  baseUrl ? null,
}:

let
  version = (fromTOML (builtins.readFile "${src}/crates/urlwasher-wasm/Cargo.toml")).package.version;

  wasm = rustPlatform.buildRustPackage {
    pname = "urldebloater-urlwasher-wasm";
    inherit version src cargoLock;

    doCheck = false;
    nativeBuildInputs = [
      lld
      wasmBindgenCli
    ];

    buildPhase = ''
      runHook preBuild
      cargo build --offline --release --target wasm32-unknown-unknown -p urldebloater-urlwasher-wasm
      runHook postBuild
    '';

    installPhase = ''
      runHook preInstall
      mkdir -p $out
      wasm-bindgen --target web --out-dir $out \
        --out-name urldebloater_urlwasher_wasm --no-typescript \
        target/wasm32-unknown-unknown/release/urldebloater_urlwasher_wasm.wasm
      runHook postInstall
    '';
  };
in
stdenv.mkDerivation {
  pname = "urldebloater-web";
  inherit version src;

  nativeBuildInputs = [ zola ];
  env.SSL_CERT_FILE = "${cacert}/etc/ssl/certs/ca-bundle.crt";
  MIXER_URL = mixerUrl;

  buildPhase = ''
    runHook preBuild
    rm -rf web/static/wasm web/public
    mkdir -p web/static/wasm
    cp ${wasm}/urldebloater_urlwasher_wasm.js web/static/wasm/
    cp ${wasm}/urldebloater_urlwasher_wasm_bg.wasm web/static/wasm/
    zola --root web build ${
      lib.optionalString (baseUrl != null) "--base-url ${lib.escapeShellArg baseUrl}"
    }
    runHook postBuild
  '';

  installPhase = ''
    runHook preInstall
    mkdir -p $out
    cp -r web/public/. $out/
    runHook postInstall
  '';

  meta = {
    description = "UrlDebloater landing page and in-browser cleaner";
    homepage = "https://github.com/makindotcc/UrlDebloater";
  };
}
