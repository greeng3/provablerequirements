---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-de744a6a5ead",
  "title": "Default network binding",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: tSeX2_vHNbMg4crEQzYrXCGmz65lH-ytw-LjM8xihUw="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.20",
  "legacy": {
    "doorstopUid": "DEPLOY-networkBinding"
  }
}
---
ReqForge shall bind its web UI to 0.0.0.0 by default, so that a
container's published port is reachable from the host and, when
the operator chooses to publish the port on a host network,
from other machines on that network. Operators who want tighter
access control shall use container networking, host firewall
rules, or reverse-proxy placement — not ReqForge-internal
controls. The default port is 36743 per DEPLOY-defaultPort.
