import { useTranslation } from 'react-i18next'
import styles from './AboutPage.module.css'

export default function AboutPage() {
  const { t } = useTranslation()

  return (
    <div className={styles.container}>
      <h1>{t('about.title')}</h1>
      
      <section className={styles.section}>
        <h2>City Challenge</h2>
        <p>{t('about.description')}</p>
      </section>

      <section className={styles.section}>
        <h3>{t('about.author')}</h3>
        <p>troissix</p>
        <p>{t('about.copyright')}</p>
      </section>

      <section className={styles.section}>
        <h3>{t('about.license')}</h3>
        <p>
          {t('about.licenseText')}{' '}
          <a
            href="https://creativecommons.org/licenses/by-sa/4.0/legalcode"
            target="_blank"
            rel="noopener noreferrer"
          >
            CC-BY-SA 4.0
          </a>
        </p>
      </section>

      <section className={styles.section}>
        <h3>{t('about.dataSource')}</h3>
        <p>
          {t('about.dataSourceText')}{' '}
          <a
            href="https://www.openstreetmap.org"
            target="_blank"
            rel="noopener noreferrer"
          >
            {t('about.openstreetmap')}
          </a>
        </p>
      </section>

      <section className={styles.section}>
        <h3>GitHub</h3>
        <p>
          <a
            href="https://www.github.com/trois-six/city-challenge"
            target="_blank"
            rel="noopener noreferrer"
          >
            github.com/trois-six/city-challenge
          </a>
        </p>
      </section>

      <section className={styles.section}>
        <h3>{t('about.lastUpdate')}</h3>
        <p>{new Date().toLocaleDateString()}</p>
      </section>
    </div>
  )
}
