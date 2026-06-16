import { useEffect, useRef } from 'react'
import L from 'leaflet'
import 'leaflet/dist/leaflet.css'
import { useTranslation } from 'react-i18next'
import { PathData } from '@/types'
import styles from './CityMap.module.css'

function pinIcon(color: string, label: string) {
  return L.divIcon({
    className: '',
    html: `<div style="
      background:${color};color:#fff;font-weight:bold;font-size:11px;
      width:28px;height:28px;border-radius:50% 50% 50% 0;
      transform:rotate(-45deg);display:flex;align-items:center;
      justify-content:center;border:2px solid rgba(0,0,0,.35);
      box-shadow:0 2px 4px rgba(0,0,0,.4)">
      <span style="transform:rotate(45deg)">${label}</span>
    </div>`,
    iconSize: [28, 28],
    iconAnchor: [14, 28],
  })
}

export default function CityMap({ pathData }: { pathData: PathData }) {
  const mapRef = useRef<HTMLDivElement>(null)
  const map = useRef<L.Map | null>(null)
  const { t } = useTranslation()

  useEffect(() => {
    if (!mapRef.current || !pathData?.coordinates?.length) return

    if (!map.current) {
      const [centerLat, centerLng] = pathData.coordinates[0]
      map.current = L.map(mapRef.current).setView([centerLat, centerLng], 13)
      L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '© OpenStreetMap contributors',
        maxZoom: 19,
      }).addTo(map.current)
    }

    const latlngs: L.LatLngTuple[] = pathData.coordinates.map(([lat, lng]) => [lat, lng])

    const polyline = L.polyline(latlngs, {
      color: '#0066cc',
      weight: 3,
      opacity: 0.8,
    }).addTo(map.current)

    const start = latlngs[0]
    const end = latlngs[latlngs.length - 1]
    L.marker(start, { icon: pinIcon('#22a722', t('map.startLabel')), title: t('map.startTitle') }).addTo(map.current)
    L.marker(end, { icon: pinIcon('#cc2200', t('map.endLabel')), title: t('map.endTitle') }).addTo(map.current)

    map.current.fitBounds(polyline.getBounds().pad(0.1))
  }, [pathData, t])

  return <div ref={mapRef} className={styles.map} />
}
