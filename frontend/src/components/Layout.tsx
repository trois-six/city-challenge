import { Outlet, Link, useLocation } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import styles from './Layout.module.css'

export default function Layout() {
  const { i18n, t } = useTranslation()
  const location = useLocation()

  const isActive = (path: string) =>
    location.pathname === path || (path === '/cities' && location.pathname === '/')

  return (
    <div className={styles.root}>
      <header className={styles.appbar}>
        <Link to="/cities" className={styles.brand}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="13" cy="4" r="2" />
            <path d="M6 21l3-6 4 2 2 4M9 15l-1-5 5-2 3 4 3 1" />
          </svg>
          CITY CHALLENGE
        </Link>

        <nav className={styles.tabs}>
          <Link to="/cities"      className={`${styles.tab} ${isActive('/cities')      ? styles.active : ''}`}>{t('cities.title')}</Link>
          <Link to="/leaderboard" className={`${styles.tab} ${isActive('/leaderboard') ? styles.active : ''}`}>{t('leaderboard.title')}</Link>
          <Link to="/players"     className={`${styles.tab} ${isActive('/players')     ? styles.active : ''}`}>{t('players.title')}</Link>
          <Link to="/about"       className={`${styles.tab} ${isActive('/about')       ? styles.active : ''}`}>{t('about.title')}</Link>
        </nav>

        <span className={styles.spacer} />

        <button
          className={styles.langBtn}
          onClick={() => i18n.changeLanguage(i18n.language === 'fr' ? 'en' : 'fr')}
          aria-label="Toggle language"
        >
          {i18n.language.toUpperCase()}
        </button>
      </header>

      <main className={styles.main}>
        <Outlet />
      </main>

      <footer className={styles.footer}>
        <div className={styles.footInner}>
          <span>CITY CHALLENGE · © 2026 troissix</span>
          <span>
            <Link to="/about">{t('about.title')}</Link>
            {' · '}
            <Link to="/health">{t('health.title')}</Link>
          </span>
        </div>
      </footer>
    </div>
  )
}
