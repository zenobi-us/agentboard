'use client';

import { useReleaseVersions } from './useReleaseVersions';

export function ReleaseVersion() {
  const releases = useReleaseVersions();
  const cliVersion = releases.status === 'loaded'
    ? releases.manifest.packages.find((releasePackage) => releasePackage.name === 'agentboard')?.version
    : undefined;

  return (
    <>v<span>{cliVersion ?? 'x.x.x'}</span></>
  );

}

