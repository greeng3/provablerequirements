---
{
  "schemaVersion": 1,
  "uuid": "019df9d6-cbf6-7ac2-8a79-e5e6bb662f4f",
  "title": "Browser support",
  "shape": "content",
  "createdAt": "2026-05-05T20:31:36.949769507Z",
  "modifiedAt": "2026-05-05T20:31:36.949769507Z",
  "links": [],
  "reviewLog": [
    {
      "timestamp": "2026-05-05T20:31:36.949769507Z",
      "reviewer": "imported-from-doorstop",
      "outcome": "approved",
      "explanation": "Imported from doorstop; original reviewed hash: r1x_2IFsiraAddsdoV1DmECIqsbOLtvc1p0Syc4ewTQ="
    }
  ],
  "active": true,
  "derived": false,
  "outlineLevel": "1.26",
  "legacy": {
    "doorstopUid": "UX-browserSupport"
  }
}
---
ReqForge's web UI shall target the current stable releases of
the major evergreen desktop browsers — Firefox, Chrome,
Safari, and Edge. Internet Explorer is explicitly not
supported. ReqForge does not commit to a specific minimum
version of any browser; users are expected to run a
reasonably current release. The documentation shall note this
expectation prominently, advising that anyone seeing UI
misbehaviour update to a current mainstream browser before
filing a bug. Mobile browsers are not an optimisation target;
the UI is built for desktop-scale use.
