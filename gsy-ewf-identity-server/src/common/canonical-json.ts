/**
 * THE ONE serialisation used to produce the bytes a credential signature commits to.
 *
 * Both ends of the signature must agree byte-for-byte, so both ends must call this and
 * nothing else. Before this existed, `issueCredential` signed `JSON.stringify(credential)`
 * (insertion order) while `verifyCredential` recovered over
 * `JSON.stringify(credentialWithoutProof, Object.keys(credentialWithoutProof).sort())`
 * (plan §0.5 bug B). That was two defects, and only one of them was the obvious one:
 *
 *  1. The two strings differed, so nothing the service issued could ever verify.
 *  2. An ARRAY REPLACER IS APPLIED AT EVERY NESTING LEVEL, not just the top. The replacer
 *     was the list of *top-level* keys, so every nested key that did not coincidentally
 *     share a top-level name was dropped: the verify-side string for a
 *     `GSYDexAddressCredential` was `..."credentialSubject":{"id":"..."}` with
 *     `accountLink.gsyDexAddress` gone. Aligning both ends on that form would have "fixed"
 *     defect 1 while shipping signatures that bind nothing but the subject DID - a silent
 *     failure, strictly worse than the loud one.
 *
 * Hence a RECURSIVE key sort, not a replacer.
 *
 * SCOPE. This is a subset of JCS (RFC 8785). JCS additionally pins number serialisation
 * (ECMAScript `Number::toString`) and string escaping (\u form for control characters).
 * Recursive key sorting is sufficient for the credential shapes actually in use - strings,
 * arrays and nested objects, no floats - and the difference only shows up for inputs this
 * service does not produce. Everything that decides the signed bytes lives in this file, so
 * upgrading to full JCS later is a change to `canonicalize()` alone and to no call site.
 * Such an upgrade would be signature-breaking, exactly as this change is.
 *
 * WHAT IS DELIBERATELY REJECTED: `undefined`, functions, symbols, `NaN`/`Infinity` and
 * cycles. `JSON.stringify` silently drops the first three from objects, turns them into
 * `null` inside arrays, and emits `null` for the numbers - all of which would let a claim
 * vanish from the signed bytes without anyone noticing, which is precisely defect 2 in a
 * different costume. Throwing is the point.
 */

/** Thrown for input that cannot be canonicalised without losing information. */
export class CanonicalizationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'CanonicalizationError';
  }
}

/**
 * Deterministic JSON serialisation: object keys sorted recursively at every depth, array
 * order left exactly as given.
 *
 * Array order is data, never sorted: `type: ['VerifiableCredential', 'FedecomAssetCredential']`
 * has a required first element per the W3C VC data model, and `@context` ordering is
 * semantically significant in JSON-LD.
 */
export function canonicalize(value: unknown): string {
  return write(value, [], '');
}

/**
 * @param ancestors objects currently open on the recursion stack, for cycle detection.
 *   An array (identity comparison over <10 frames) beats a Set here and, unlike a plain
 *   "seen" Set, does not reject the same object legitimately appearing twice in siblings.
 * @param path JSON-pointer-ish location, so a rejection names the offending field.
 */
function write(value: unknown, ancestors: object[], path: string): string {
  const at = path === '' ? 'the root value' : `'${path}'`;

  if (value === null) {
    return 'null';
  }

  if (value === undefined) {
    throw new CanonicalizationError(
      `cannot canonicalize undefined at ${at}: JSON.stringify would silently drop it, ` +
        'so the signature would not cover it. Omit the field or use null explicitly.',
    );
  }

  switch (typeof value) {
    case 'boolean':
      return value ? 'true' : 'false';

    case 'number':
      if (!Number.isFinite(value)) {
        throw new CanonicalizationError(
          `cannot canonicalize ${String(value)} at ${at}: JSON.stringify emits null for it.`,
        );
      }
      // Matches JSON.stringify. Full JCS pins this same ECMAScript number-to-string
      // conversion, so this line is already JCS-conformant.
      return JSON.stringify(value);

    case 'string':
      // Escaping delegated to JSON.stringify; see the SCOPE note above.
      return JSON.stringify(value);

    case 'bigint':
      throw new CanonicalizationError(
        `cannot canonicalize a bigint at ${at}: JSON.stringify throws on it and JSON has ` +
          'no bigint representation. Serialise it as a string first.',
      );

    case 'function':
    case 'symbol':
      throw new CanonicalizationError(
        `cannot canonicalize a ${typeof value} at ${at}: JSON.stringify would silently ` +
          'drop it, so the signature would not cover it.',
      );

    case 'object':
      break;

    default:
      throw new CanonicalizationError(`cannot canonicalize a ${typeof value} at ${at}.`);
  }

  const object = value as Record<string, unknown>;

  if (ancestors.includes(object)) {
    throw new CanonicalizationError(
      `cannot canonicalize a circular structure: ${at} refers back to one of its ancestors.`,
    );
  }

  // Same `toJSON` contract as `JSON.stringify`, so a Date or a Mongoose document
  // canonicalises to what it would serialise to. The result is canonicalised in turn.
  const toJSON = (object as { toJSON?: unknown }).toJSON;
  if (typeof toJSON === 'function') {
    return write((toJSON as (key?: string) => unknown).call(object), ancestors, path);
  }

  const nested = [...ancestors, object];

  if (Array.isArray(object)) {
    // ORDER PRESERVED - see the doc comment.
    const items = object.map((item, index) => write(item, nested, `${path}[${index}]`));
    return `[${items.join(',')}]`;
  }

  // THE recursive sort. `Object.keys` gives own enumerable string keys in insertion order
  // (integer-like keys first, per the ES spec); sorting removes that dependency entirely.
  // `sort()` with no comparator is UTF-16 code-unit order, which is what JCS specifies.
  const keys = Object.keys(object).sort();
  const members = keys.map((key) => {
    const child = (object as Record<string, unknown>)[key];
    const childPath = path === '' ? key : `${path}.${key}`;
    return `${JSON.stringify(key)}:${write(child, nested, childPath)}`;
  });

  return `{${members.join(',')}}`;
}
