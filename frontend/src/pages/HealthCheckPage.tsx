import { useEffect, useState } from 'react'
import { useTranslation } from 'react-i18next'
import styles from './HealthCheckPage.module.css'

interface HealthStatus {
  timestamp: string
  healthy: boolean
  environment: string
  checks: {
    jsEnabled: boolean
    reactRendering: boolean
  }
}

export default function HealthCheckPage() {
  const { t } = useTranslation()
  const [health, setHealth] = useState<HealthStatus | null>(null)

  useEffect(() => {
    const checks = {
      jsEnabled: typeof window !== 'undefined',
      reactRendering: true,
    }

    setHealth({
      timestamp: new Date().toISOString(),
      healthy: Object.values(checks).every((value) => value),
      environment: import.meta.env.MODE,
      checks,
    })
  }, [])

  if (!health) return null

  const checkLabels: Record<keyof HealthStatus['checks'], string> = {
    jsEnabled: t('health.checkJsEnabled'),
    reactRendering: t('health.checkReactRendering'),
  }

  return (
    <div className={styles.container}>
      <h1>{t('health.title')}</h1>

      <div className={health.healthy ? styles.statusOk : styles.statusError}>
        <h2>{health.healthy ? t('health.healthy') : t('health.unhealthy')}</h2>
        <p>
          <strong>{t('health.timestamp')}:</strong>{' '}
          {new Date(health.timestamp).toLocaleString()}
        </p>
        <p>
          <strong>{t('health.environment')}:</strong> {health.environment}
        </p>
      </div>

      <div className={styles.checks}>
        <h3>{t('health.checksTitle')}</h3>
        <ul className={styles.checkList}>
          {Object.entries(health.checks).map(([key, value]) => (
            <li key={key} className={value ? styles.checkOk : styles.checkError}>
              <span>{checkLabels[key as keyof HealthStatus['checks']]}</span>
              <strong>{value ? t('health.ok') : t('health.failed')}</strong>
            </li>
          ))}
        </ul>
      </div>

      <button className={styles.refreshButton} onClick={() => window.location.reload()}>
        {t('health.refresh')}
      </button>
    </div>
  )
}
