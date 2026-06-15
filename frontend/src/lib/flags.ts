/**
 * ISO 3166-1 alpha-3 to alpha-2 country code mapping, limited to the
 * countries used across the City Challenge dataset and a broader set of
 * common nationalities.
 */
const ISO3_TO_ISO2: Record<string, string> = {
  FRA: 'FR',
  BEL: 'BE',
  GBR: 'GB',
  DEU: 'DE',
  ITA: 'IT',
  ESP: 'ES',
  USA: 'US',
  NLD: 'NL',
  PRT: 'PT',
  CHE: 'CH',
  AUT: 'AT',
  LUX: 'LU',
  IRL: 'IE',
  CAN: 'CA',
  MAR: 'MA',
  DZA: 'DZ',
  TUN: 'TN',
  POL: 'PL',
  SWE: 'SE',
  NOR: 'NO',
  DNK: 'DK',
  FIN: 'FI',
  GRC: 'GR',
  TUR: 'TR',
  JPN: 'JP',
  CHN: 'CN',
  BRA: 'BR',
  MEX: 'MX',
  AUS: 'AU',
  NZL: 'NZ',
}

const FLAG_OFFSET = 127397 // 0x1F1E6 ('A' regional indicator) - 'A'.charCodeAt(0)
const UNKNOWN_FLAG = '🏳️'

/**
 * Convert an ISO 3166-1 alpha-3 country code to its flag emoji.
 * Falls back to a white flag for unknown codes.
 */
export function getFlagEmoji(countryCode: string): string {
  const iso2 = ISO3_TO_ISO2[countryCode.toUpperCase()]
  if (!iso2) return UNKNOWN_FLAG

  return [...iso2].map((char) => String.fromCodePoint(FLAG_OFFSET + char.charCodeAt(0))).join('')
}
