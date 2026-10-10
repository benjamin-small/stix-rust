# Hackers (1995) dataset

A STIX 2.1 dataset that models the plot of the film *Hackers* (1995, United Artists; written by Rafael Moreno, directed by Iain Softley). It is a fan-made illustration for testing and demonstrating `stix-rust`. It is not affiliated with or endorsed by the film's makers or studio. The narrative text is original.

## Files

| File | Contents |
| --- | --- |
| `bundle.json` | One STIX 2.1 bundle (`bundle--546eb18c-c7d9-43f0-9ca1-c9332b0e8d42`) with 75 objects. The `report` object carries a Markdown narrative. |
| `patterns.json` | Example STIX patterns, each with the `observed-data` objects it is expected to match (or not). |

## Cast and objects

Every non-relationship object in the bundle, with its id. The 35 `relationship` objects are not listed. This table was generated from `bundle.json`.

| STIX type | Name or value | Id |
| --- | --- | --- |
| threat-actor | Zero Cool (aliases: Zero Cool, Crash Override, Dade Murphy) | `threat-actor--35817f6b-3bee-455b-9293-1b675c3e039f` |
| threat-actor | Acid Burn (aliases: Acid Burn, Kate Libby) | `threat-actor--769697f8-5d89-4852-b0ca-35007a113207` |
| threat-actor | Cereal Killer (aliases: Cereal Killer, Emmanuel Goldstein) | `threat-actor--9bece18c-6a60-4388-ad76-b17b87bc341c` |
| threat-actor | Lord Nikon (aliases: Lord Nikon, Paul Cook) | `threat-actor--c244d45b-bd59-4aa1-9654-31fa5e838060` |
| threat-actor | Phantom Phreak (aliases: Phantom Phreak, Ramón Sánchez) | `threat-actor--2089251f-5830-4ec5-acb7-75d95df5fe73` |
| threat-actor | Joey Pardella (aliases: Joey Pardella) | `threat-actor--1fb8f593-d33d-48a4-8552-c2d9c1866c69` |
| threat-actor | The Plague (aliases: The Plague, Eugene Belford) | `threat-actor--33cb320c-1869-4419-ac56-2baaffa7d8b8` |
| intrusion-set | The Elite | `intrusion-set--8a20f35b-7222-47f9-a508-413067db5251` |
| campaign | The Plague's frame-up | `campaign--8a4cc02b-ae30-4990-83c8-064bbd5c0740` |
| campaign | Hack the Planet | `campaign--2b8c0f39-3568-4a89-8fbd-258815f30a9e` |
| malware | Salami-slicing worm | `malware--4ca60a83-79b5-4ae9-a447-85c67318c125` |
| malware | Da Vinci | `malware--b646305f-de43-4fd2-8bdf-59d2b74f4fb7` |
| attack-pattern | Phishing for Information | `attack-pattern--af6b93c5-2f3a-4790-9eb6-10a48e433f15` |
| attack-pattern | Password Guessing | `attack-pattern--8c607087-cbb0-4f32-a221-d83435f5693f` |
| attack-pattern | Exfiltration Over C2 Channel | `attack-pattern--a00dcef2-1601-4b4f-b648-0bb7acc2ae34` |
| attack-pattern | Stored Data Manipulation | `attack-pattern--7625cc32-dbbc-4f4a-93cf-3292f98f0b66` |
| tool | Dial-up modem | `tool--7e7e808f-37c7-4254-8575-14e7020d6a4b` |
| infrastructure | Gibson | `infrastructure--ef2790ea-e944-4c32-8e2e-62b404bd072c` |
| identity | Ellingson Mineral Company | `identity--1811a3f4-5206-48e0-b9b3-1f4a739c38b9` |
| identity | Richard Gill | `identity--4dd7a523-5d73-42c2-83bd-63f91d3f9ebd` |
| location | New York City | `location--2bb7b5a8-54ea-4cb9-a7f1-3b3bbe9e9825` |
| vulnerability | Gibson weak account passwords | `vulnerability--76711ef5-f535-45fa-8dd7-c7b40f0d7c18` |
| course-of-action | Remove the worm and reconcile accounts | `course-of-action--d517378f-b284-404d-8041-494d3a9b5a34` |
| course-of-action | Replace common passwords on the Gibson | `course-of-action--b790ecf5-0aba-441a-8ae0-b249d49c7310` |
| indicator | Garbage file SHA-256 | `indicator--fa96607e-72ac-4619-8d53-1d6db18e9291` |
| indicator | Garbage file location on the Gibson | `indicator--6c5ca72a-3d7f-42c5-b4a7-2d57e10ecbdd` |
| observed-data | (unnamed) | `observed-data--2d7d303d-769d-487d-8ff5-13e4e0462cd9` |
| observed-data | (unnamed) | `observed-data--351e35e5-5273-49ed-b2d5-16a4e951f29b` |
| observed-data | (unnamed) | `observed-data--0d9e70c2-76d1-4ba2-b615-68b0d6aa87f8` |
| sighting | (unnamed) | `sighting--3cf2ec0a-7cfe-42ca-b846-7fe7f17bfa23` |
| sighting | (unnamed) | `sighting--f1b8d69b-ae86-438c-997c-8cf131f56a31` |
| grouping | Garbage file evidence | `grouping--982660d8-2c4b-4cd9-8b8b-7170319d4fde` |
| note | What the garbage file holds | `note--c87caa11-c40d-4e2b-b83a-ef2655992450` |
| opinion | (unnamed) | `opinion--74fee950-76af-464b-82ac-b1427f958c6f` |
| report | Hackers (1995): the Ellingson case file | `report--5b8eed6a-8ced-46b3-aee2-9fd855d2bae6` |
| ipv4-addr | 192.0.2.15 | `ipv4-addr--e8e95628-3634-51a1-8be2-f5d03ba24592` |
| domain-name | ellingson-mineral.example | `domain-name--90a9c62a-f2c5-5502-b8d2-2ee057918911` |
| url | http://ellingson-mineral.example/gibson/garbage | `url--4782a3f0-2e66-5581-8015-318663e7d4af` |
| file | garbage | `file--46b8a83f-6581-5e9f-849e-137413562714` |
| artifact | (unnamed) | `artifact--85e3e46e-bf42-59d5-9d61-158923618e42` |

## Conventions

- **Handles.** The threat-actor is named "Zero Cool", with "Crash Override" in `aliases`. The narrative uses each handle for its period: Zero Cool for 1988, Crash Override for 1995.
- **Attribution direction.** The Elite (`intrusion-set`) is `attributed-to` each crew member (`threat-actor`), which is the direction STIX 2.1 defines.
- **Ellingson and the Gibson.** Ellingson Mineral Company is `related-to` the Gibson, because STIX does not define an identity `owns` infrastructure relationship.
- **Reserved addresses.** Domains use the `.example` TLD. IPv4 addresses come from the RFC 5737 documentation ranges (`192.0.2.0/24`, `198.51.100.0/24`, `203.0.113.0/24`).
- **File hash.** The `file` object's `hashes.SHA-256` is the real SHA-256 of the decoded `payload_bin` of its `content_ref` `artifact`.
- **SCO ids.** Cyber-observable ids are deterministic UUIDv5 values, as the spec requires.
- **No pattern qualifiers.** Example patterns do not use `WITHIN`, `REPEATS` or `START...STOP`.
- **Fiction.** The vulnerability has no CVE, the in-story dates are invented, and the ATT&CK techniques are approximations of what happens on screen.

## Validation

`stix-rust` checks every dataset directory with:

```sh
cargo test -p stix-rust --test datasets
```

The test checks ids, timestamps, references, graph shape and that each example pattern matches as expected.

The bundle was also checked with OASIS `stix2-validator` 3.2.0 (3.3.1 on PyPI ships without its schemas and cannot run). The result is valid. Four `{302}` warnings remain on the ATT&CK external references, which have a URL but no hash. They are expected: MITRE's own data carries the same URLs without hashes.

## Adding another movie dataset

Create a sibling directory under `datasets/` (for example `datasets/<title>-<year>/`) with the same three files: `bundle.json`, `patterns.json` and a `README.md`. The `datasets` test discovers it automatically.
