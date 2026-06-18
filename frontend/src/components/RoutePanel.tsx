import { useRef, useEffect } from 'react'
import { useTranslation } from 'react-i18next'
import { RouteStep, TurnInstruction } from '@/types'
import styles from './RoutePanel.module.css'

const TURN_ICON: Record<TurnInstruction, string> = {
  start: '●',
  straight: '↑',
  slight_left: '↖',
  slight_right: '↗',
  turn_left: '←',
  turn_right: '→',
  sharp_left: '↙',
  sharp_right: '↘',
  uturn: '↩',
  arrive: '⬤',
}

function formatDistance(meters: number): string {
  if (meters >= 1000) {
    return `${(meters / 1000).toFixed(1)} km`
  }
  return `${Math.round(meters)} m`
}

interface RoutePanelProps {
  steps: RouteStep[]
  activeIndex: number | null
  onStepSelect: (index: number) => void
}

export default function RoutePanel({ steps, activeIndex, onStepSelect }: RoutePanelProps) {
  const { t } = useTranslation()
  const listRef = useRef<HTMLUListElement>(null)
  const activeRef = useRef<HTMLLIElement>(null)

  useEffect(() => {
    activeRef.current?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
  }, [activeIndex])

  const instructionLabel = (step: RouteStep): string => {
    const base = t(`route.${step.instruction}`)
    if (step.instruction === 'arrive' || step.instruction === 'start') return base
    if (step.streetName) return `${base} ${t('route.on')} ${step.streetName}`
    return base
  }

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <span className={styles.title}>{t('route.panelTitle')}</span>
        <span className={styles.count}>{t('route.totalSteps', { count: steps.length - 1 })}</span>
      </div>

      <ul className={styles.list} ref={listRef}>
        {steps.map((step, i) => (
          <li
            key={i}
            ref={i === activeIndex ? activeRef : null}
            className={`${styles.step} ${i === activeIndex ? styles.active : ''}`}
            onClick={() => onStepSelect(i)}
          >
            <span className={styles.icon} aria-hidden="true">
              {TURN_ICON[step.instruction]}
            </span>
            <span className={styles.text}>
              <span className={styles.instruction}>{instructionLabel(step)}</span>
              {step.address && <span className={styles.address}>{step.address}</span>}
              {step.distanceM > 0 && (
                <span className={styles.distance}>{formatDistance(step.distanceM)}</span>
              )}
            </span>
          </li>
        ))}
      </ul>
    </div>
  )
}
