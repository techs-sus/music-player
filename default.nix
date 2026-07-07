{
  lib,
  rustPlatform,
  ...
}:
rustPlatform.buildRustPackage {
  pname = "music-player";
  version = "0.1.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  buildFeatures = [ ];

  nativeBuildInputs = [ ];
  buildInputs = [ ];

  meta = {
    description = "lightweight music player";
    homepage = "https://github.com/techs-sus/music";
    license = lib.licenses.asl20; # apache license 2.0
    maintainers = [
      {
        name = "techs-sus";
        github = "techs-sus";
        githubId = 92276908;
      }
    ];
    platforms = lib.platforms.unix;
    mainProgram = "music-player";
  };
}
