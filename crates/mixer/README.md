# mixer

Rest api for washing URLs. \
Main goal is to hide real user IP when unrolling short links (like https://vm.tiktok.com/uniqueGeneratedIdIdentifyingUserWhoSharedIt)\
Second one is providing easy way to clean URL from an iOS shortcut.

## Running
Mixer assumes you are running behind proxy where header ``X-Forwarded-For`` cannot be spoofed. 

## Endpoints

### /wash?url={DIRTY_URL}

#### Request

Method: GET

#### Response

Success:
- Status OK (200)
- Body contains raw text with cleaned url

Failures:

- ratelimited (status 429)
- invalid URL (bad request, status 400)
