import { describe, it, expect } from 'vitest'
import { getFlagEmoji } from './flags'

describe('getFlagEmoji', () => {
  it('converts known ISO 3166-1 alpha-3 codes to flag emojis', () => {
    expect(getFlagEmoji('FRA')).toBe('🇫🇷')
    expect(getFlagEmoji('GBR')).toBe('🇬🇧')
    expect(getFlagEmoji('USA')).toBe('🇺🇸')
    expect(getFlagEmoji('DEU')).toBe('🇩🇪')
  })

  it('is case-insensitive', () => {
    expect(getFlagEmoji('fra')).toBe('🇫🇷')
  })

  it('falls back to a white flag for unknown codes', () => {
    expect(getFlagEmoji('XXX')).toBe('🏳️')
  })
})
