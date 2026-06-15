import { useEffect, useRef } from 'react'
import L from 'leaflet'
import 'leaflet/dist/leaflet.css'
import { PathData } from '@/types'
import styles from './CityMap.module.css'

export default function CityMap({ pathData }: { pathData: PathData }) {
  const mapRef = useRef<HTMLDivElement>(null)
  const map = useRef<L.Map | null>(null)

  useEffect(() => {
    if (!mapRef.current || !pathData?.coordinates) return

    // Initialize map
    if (!map.current) {
      const centerLat = pathData.coordinates[0][0]
      const centerLng = pathData.coordinates[0][1]

      map.current = L.map(mapRef.current).setView([centerLat, centerLng], 13)

      L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
        attribution: '© OpenStreetMap contributors',
        maxZoom: 19,
      }).addTo(map.current)
    }

    // Draw the path
    if (map.current && pathData.coordinates.length > 0) {
      const latlngs: L.LatLngTuple[] = pathData.coordinates.map((coord) => [coord[0], coord[1]])
      L.polyline(latlngs, {
        color: '#0066cc',
        weight: 3,
        opacity: 0.8,
      }).addTo(map.current)

      // Fit bounds
      const group = new L.FeatureGroup(
        latlngs.map((coord) => L.marker(coord))
      )
      map.current.fitBounds(group.getBounds().pad(0.1))
    }
  }, [pathData])

  return <div ref={mapRef} className={styles.map} />
}
