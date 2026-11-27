import {parseDocument, visit} from 'yaml'

export function record(value) {
  return value !== null && typeof value === 'object' && !Array.isArray(value)
}

export function label(key) {
  const words = String(key).replaceAll('_', ' ')
  const names = {a2a:'A2A', id:'ID', url:'URL', http:'HTTP', yaml:'YAML', ms:'ms', usd:'USD'}
  return words.split(' ').map((word, i) => names[word] || (i === 0 ? word[0]?.toUpperCase() + word.slice(1) : word)).join(' ')
}

export function parseProfile(source) {
  if (source.length > 262144) throw Error('The specification is too large for a structured profile.')
  const doc = parseDocument(source, {
    schema:'core', stringKeys:true, resolveKnownTags:false, prettyErrors:false,
    logLevel:'silent', intAsBigInt:true,
  })
  if (doc.errors.length || doc.warnings.length) throw Error('The YAML contains unsupported syntax.')
  let nodes = 0
  visit(doc, (key, node, path) => {
    if (++nodes > 5000 || path.length > 48) throw Error('The specification exceeds the profile display limits.')
  })
  const value = doc.toJS({maxAliasCount:32})
  let expanded = 0
  function check(value, depth) {
    if (++expanded > 5000 || depth > 24) throw Error('The specification exceeds the profile display limits.')
    if (value && typeof value === 'object') {
      for (const child of Object.values(value)) check(child, depth + 1)
    }
  }
  check(value, 0)
  if (!record(value) || !record(value.agent)) throw Error('The YAML has no agent record.')
  return value
}
