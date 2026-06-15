import { Outlet, Link, useLocation } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import styles from './Layout.module.css'

export default function Layout() {
  const { i18n, t } = useTranslation()
  const location = useLocation()

  const toggleLanguage = () => {
    i18n.changeLanguage(i18n.language === 'fr' ? 'en' : 'fr')
  }

  const isActive = (path: string) => {
    const currentPath = location.pathname.replace(/^#/, '')
    return currentPath === path || (path === '/' && currentPath === '')
  }

  return (
    <div className={styles.container}>
      <header className={styles.header}>
        <div className={styles.headerContent}>
          <div className={styles.logo}>
            <h1>{t('header.title')}</h1>
            <p className={styles.subtitle}>{t('header.subtitle')}</p>
          </div>
          <button
            className={styles.languageButton}
            onClick={toggleLanguage}
            aria-label={t('header.language')}
          >
            {i18n.language.toUpperCase()}
          </button>
        </div>
        <nav className={styles.topNav}>
          <Link
            to="/cities"
            className={`${styles.navLink} ${
              isActive('/cities') ? styles.active : ''
            }`}
          >
            {t('nav.cities')}
          </Link>
          <Link
            to="/players"
            className={`${styles.navLink} ${
              isActive('/players') ? styles.active : ''
            }`}
          >
            {t('nav.players')}
          </Link>
          <Link
            to="/leaderboard"
            className={`${styles.navLink} ${
              isActive('/leaderboard') ? styles.active : ''
            }`}
          >
            {t('nav.leaderboard')}
          </Link>
        </nav>
      </header>

      <main className={styles.main}>
        <Outlet />
      </main>

      <footer className={styles.footer}>
        <p>
          City Challenge © 2026 troissix |{' '}
          <Link to="/a-propos">{t('nav.about')}</Link>
        </p>
      </footer>
    </div>
  )
}
