import { createHashRouter } from 'react-router-dom'
import Layout from '@/components/Layout'
import CitiesPage from '@/pages/CitiesPage'
import CityDetailPage from '@/pages/CityDetailPage'
import PlayersPage from '@/pages/PlayersPage'
import PlayerDetailPage from '@/pages/PlayerDetailPage'
import LeaderboardPage from '@/pages/LeaderboardPage'
import HealthCheckPage from '@/pages/HealthCheckPage'
import AboutPage from '@/pages/AboutPage'

const router = createHashRouter([
  {
    path: '/',
    element: <Layout />,
    children: [
      {
        path: '/',
        element: <CitiesPage />,
      },
      {
        path: '/cities',
        element: <CitiesPage />,
      },
      {
        path: '/city/:id',
        element: <CityDetailPage />,
      },
      {
        path: '/players',
        element: <PlayersPage />,
      },
      {
        path: '/player/:id',
        element: <PlayerDetailPage />,
      },
      {
        path: '/leaderboard',
        element: <LeaderboardPage />,
      },
      {
        path: '/health',
        element: <HealthCheckPage />,
      },
      {
        path: '/about',
        element: <AboutPage />,
      },
    ],
  },
])

export default router
