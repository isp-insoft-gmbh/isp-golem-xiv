# Seats and cost

A seat is one person's subscription at one provider.
Seats are not pooled, and one seat pays for each run.

## Register a seat

Open [Seats](https://isp-golem-xiv.isp-insoft-gmbh.workers.dev/seats), sign in with GitHub as an organization member, and follow the instructions for Anthropic or OpenAI Codex.
Use a fresh sign-in only for golems, not the one you use in your own CLI: refresh can invalidate the other copy.
API keys are not supported.

The browser encrypts the sign-in for the private ops repository before sending it.
The router relays ciphertext and cannot read the sign-in.
Registering the same provider again replaces its sign-in.
Register a fresh one if it expires or is revoked; the dashboard cannot warn before expiry.

## Who pays

| Trigger | Seat holder |
| --- | --- |
| Mention or **Run now** by an organization member | That member; without a seat the run is refused. |
| Event caused by an organization member | That member if they have a seat, otherwise the golem's owner. |
| Schedule, bot, or CI | The golem's owner. |

Only organization members can hold seats or start a golem by mention or **Run now**.
You are responsible for using your subscription within its provider's terms.

## Which provider runs

Anthropic seats support Claude and Pi; OpenAI Codex seats support Codex and Pi.
A run first tries the requested provider on the paying person's account.
If that person has no seat at that provider, it tries their preferred registered provider.
If the requested harness cannot use the selected provider, Pi runs instead.
A fallback uses the selected harness's default model for that provider and reports what was requested, what ran, and why.
