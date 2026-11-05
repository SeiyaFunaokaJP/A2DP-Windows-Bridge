# Legal notes

## Clean-room statement

This repository is an **independent, clean-room reimplementation** of the aptX
Adaptive codec and its A2DP transport for Linux. The implementation was
developed without access to any Qualcomm source code, software development kit,
NDA-protected material, confidential documentation, or leaked vendor source.
Nothing in this repository was supplied by the vendor, under NDA or otherwise.

Every line of source code here was written by the project author. Numeric
constants (filter coefficients, quantiser tables, Huffman/Golomb-Rice tables,
bit-allocation tables, prototype coefficients) were independently reconstructed
by static analysis and measurement of lawfully obtained shipped binaries, and
by black-box observation of on-air data. They were not copied from any vendor
header, source file, or SDK.

## Sources of information

1. **Public specifications and public source code** — Bluetooth SIG A2DP
   profile definitions and the publicly available AOSP Bluetooth stack's codec
   identifiers, capability layouts and constants.
2. **Static reverse engineering** — disassembly and data analysis of publicly
   distributed firmware and encoder libraries (Hexagon ELF modules) for the
   purpose of interoperability.
3. **Black-box measurement** — captures of A2DP traffic and of our own
   transmissions.
4. **Public reference decoders, verification only** — Qualcomm reference
   decoder executables, where obtained from public firmware packages, were run
   offline solely to confirm that our output decodes. They are not part of, and
   are not redistributed with, this repository.

## What is and is not included

* **Included:** original MIT-licensed code and documentation; reconstructed
  numeric tables required for interoperability.
* **Not included:** Qualcomm binaries, SDKs, headers, confidential documents,
  or source code. The Rust crate does not link against, load, or require any
  vendor library at build time.

## License

All original code and documentation is MIT-licensed; see [`LICENSE`](LICENSE).
The reconstructed numeric data is included for interoperability purposes and is
provided "as is", without warranty of any kind, express or implied, including
but not limited to warranties of merchantability, fitness for a particular
purpose, title, or non-infringement.

## Trademarks

`aptX`, `aptX Adaptive`, `aptX HD`, `aptX Low Latency` and related marks are
trademarks of Qualcomm Incorporated. This project is not affiliated with,
sponsored by, or endorsed by Qualcomm. All other trademarks are the property of
their respective owners.

## Jurisdiction and compliance

Reverse engineering for the purpose of achieving interoperability is lawful in
many jurisdictions, but the rules differ. In particular, some jurisdictions
restrict circumvention of technical protection measures, and some restrict
reverse engineering of computer programs by contract or statute. This
repository is provided for interoperability and research; **you are responsible
for determining whether your use is lawful where you are**, and you must not use
it where such use is prohibited. No confidential information, trade secret, or
NDA-protected material was used in its creation.

## Limitation of liability

THE SOFTWARE AND DATA ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT,
TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR
THE USE OR OTHER DEALINGS IN THE SOFTWARE OR DATA.
