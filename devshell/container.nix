{ name
, version
, dockerTools
, polkadot
, buildEnv
, ...
}:

dockerTools.buildImage {
  inherit name;
  tag = "v${version}";

  copyToRoot = buildEnv {
    name = "image-root";
    paths = [ polkadot ];
    pathsToLink = [ "/bin" ];
  };

  config = {
    Entrypoint = [ "${polkadot}/bin/polkadot" ];
  };
}
