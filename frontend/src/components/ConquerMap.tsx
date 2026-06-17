import { useEffect, useRef } from 'react'
import { useTranslation } from 'react-i18next'
import { PathData } from '@/types'
import styles from './ConquerMap.module.css'

interface ConquerMapProps {
  pathData: PathData
  highlightGeometry?: [number, number][]
  focusCoordinate?: [number, number]
}

interface Bounds {
  minLat: number
  maxLat: number
  minLng: number
  maxLng: number
}

const LOOP_SECONDS = 45
const FADE_SECONDS = 1.8

export default function ConquerMap({ pathData, highlightGeometry, focusCoordinate }: ConquerMapProps) {
  const { t } = useTranslation()
  const screenRef = useRef<HTMLDivElement>(null)
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const pctRef = useRef<HTMLSpanElement>(null)
  const baseCanvasRef = useRef<HTMLCanvasElement | null>(null)
  const trailCanvasRef = useRef<HTMLCanvasElement | null>(null)
  const sizeRef = useRef({ w: 0, h: 0 })
  const rafRef = useRef<number | null>(null)
  const stateRef = useRef({ headPos: 0, lastTs: 0, fading: false, fadeStart: 0 })
  const highlightRef = useRef(highlightGeometry)
  const focusRef = useRef(focusCoordinate)

  useEffect(() => {
    highlightRef.current = highlightGeometry
    focusRef.current = focusCoordinate
  }, [highlightGeometry, focusCoordinate])

  useEffect(() => {
    const screen = screenRef.current
    const canvas = canvasRef.current
    if (!screen || !canvas || !pathData?.coordinates?.length) return

    const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches

    const allLat: number[] = []
    const allLng: number[] = []
    for (const [lat, lng] of pathData.coordinates) {
      allLat.push(lat)
      allLng.push(lng)
    }
    for (const street of pathData.streetGeometries) {
      for (const [lat, lng] of street) {
        allLat.push(lat)
        allLng.push(lng)
      }
    }
    const bounds: Bounds = {
      minLat: Math.min(...allLat),
      maxLat: Math.max(...allLat),
      minLng: Math.min(...allLng),
      maxLng: Math.max(...allLng),
    }

    const ctx = canvas.getContext('2d')
    if (!ctx) return

    const project = (lat: number, lng: number, w: number, h: number): [number, number] => {
      const pad = 0.08
      const latRange = bounds.maxLat - bounds.minLat || 0.001
      const lngRange = bounds.maxLng - bounds.minLng || 0.001
      const px = (lng - bounds.minLng) / lngRange
      const py = (lat - bounds.minLat) / latRange
      const x = (pad + px * (1 - 2 * pad)) * w
      const y = (1 - (pad + py * (1 - 2 * pad))) * h
      return [x, y]
    }

    const projectIndexed = (idx: number, w: number, h: number): [number, number] => {
      const clamped = Math.max(0, Math.min(pathData.coordinates.length - 1, Math.floor(idx)))
      const [lat, lng] = pathData.coordinates[clamped]
      return project(lat, lng, w, h)
    }

    const pointAt = (pos: number, w: number, h: number): [number, number] => {
      const i0 = Math.floor(pos)
      const i1 = Math.min(pathData.coordinates.length - 1, i0 + 1)
      const frac = pos - i0
      const [lat0, lng0] = pathData.coordinates[i0]
      const [lat1, lng1] = pathData.coordinates[i1]
      return project(lat0 + (lat1 - lat0) * frac, lng0 + (lng1 - lng0) * frac, w, h)
    }

    function buildBase(w: number, h: number) {
      const base = document.createElement('canvas')
      base.width = w
      base.height = h
      const bctx = base.getContext('2d')!
      bctx.lineCap = 'round'
      bctx.lineJoin = 'round'
      bctx.strokeStyle = 'rgba(255, 74, 60, 0.20)'
      bctx.lineWidth = 2
      for (const street of pathData.streetGeometries) {
        if (street.length < 2) continue
        bctx.beginPath()
        street.forEach(([lat, lng], i) => {
          const [x, y] = project(lat, lng, w, h)
          if (i === 0) bctx.moveTo(x, y)
          else bctx.lineTo(x, y)
        })
        bctx.stroke()
      }

      const [sLat, sLng] = pathData.coordinates[0]
      const [eLat, eLng] = pathData.coordinates[pathData.coordinates.length - 1]
      const [sx, sy] = project(sLat, sLng, w, h)
      const [ex, ey] = project(eLat, eLng, w, h)
      bctx.fillStyle = '#33E07A'
      bctx.beginPath()
      bctx.arc(sx, sy, 5, 0, Math.PI * 2)
      bctx.fill()
      bctx.fillStyle = '#FF4A3C'
      bctx.beginPath()
      bctx.arc(ex, ey, 5, 0, Math.PI * 2)
      bctx.fill()
      return base
    }

    function resize() {
      const rect = screen!.getBoundingClientRect()
      const dpr = Math.min(window.devicePixelRatio || 1, 2)
      const w = Math.max(1, Math.round(rect.width * dpr))
      const h = Math.max(1, Math.round(rect.height * dpr))
      sizeRef.current = { w, h }
      canvas!.width = w
      canvas!.height = h
      baseCanvasRef.current = buildBase(w, h)
      const trail = document.createElement('canvas')
      trail.width = w
      trail.height = h
      trailCanvasRef.current = trail
      stateRef.current.headPos = 0
      stateRef.current.fading = false
    }

    resize()
    const ro = new ResizeObserver(resize)
    ro.observe(screen)

    function addTrail(from: number, to: number) {
      const trail = trailCanvasRef.current
      if (!trail) return
      const { w, h } = sizeRef.current
      const tctx = trail.getContext('2d')!

      const points: [number, number][] = [pointAt(from, w, h)]
      let idx = Math.floor(from) + 1
      while (idx < to) {
        points.push(projectIndexed(idx, w, h))
        idx++
      }
      points.push(pointAt(to, w, h))

      tctx.lineCap = 'round'
      tctx.lineJoin = 'round'

      tctx.globalCompositeOperation = 'lighter'
      tctx.strokeStyle = 'rgba(51, 224, 122, 0.35)'
      tctx.lineWidth = 6
      tctx.beginPath()
      points.forEach(([x, y], i) => (i === 0 ? tctx.moveTo(x, y) : tctx.lineTo(x, y)))
      tctx.stroke()

      tctx.globalCompositeOperation = 'source-over'
      tctx.strokeStyle = '#7BFFB0'
      tctx.lineWidth = 2
      tctx.beginPath()
      points.forEach(([x, y], i) => (i === 0 ? tctx.moveTo(x, y) : tctx.lineTo(x, y)))
      tctx.stroke()
    }

    function drawFrame() {
      const { w, h } = sizeRef.current
      ctx!.clearRect(0, 0, w, h)
      if (baseCanvasRef.current) ctx!.drawImage(baseCanvasRef.current, 0, 0)
      if (trailCanvasRef.current) ctx!.drawImage(trailCanvasRef.current, 0, 0)

      if (!highlightRef.current) {
        const [hx, hy] = pointAt(stateRef.current.headPos, w, h)
        ctx!.save()
        ctx!.shadowColor = '#33E07A'
        ctx!.shadowBlur = 14
        ctx!.fillStyle = '#F0FFF4'
        ctx!.beginPath()
        ctx!.arc(hx, hy, 5, 0, Math.PI * 2)
        ctx!.fill()
        ctx!.restore()
      }

      if (highlightRef.current && highlightRef.current.length > 1) {
        ctx!.save()
        ctx!.strokeStyle = '#FFB23D'
        ctx!.shadowColor = '#FFB23D'
        ctx!.shadowBlur = 10
        ctx!.lineWidth = 4
        ctx!.lineCap = 'round'
        ctx!.lineJoin = 'round'
        ctx!.beginPath()
        highlightRef.current.forEach(([lat, lng], i) => {
          const [x, y] = project(lat, lng, w, h)
          if (i === 0) ctx!.moveTo(x, y)
          else ctx!.lineTo(x, y)
        })
        ctx!.stroke()
        ctx!.restore()
      }

      if (focusRef.current) {
        const [x, y] = project(focusRef.current[0], focusRef.current[1], w, h)
        const pulse = 6 + 3 * Math.sin(performance.now() / 200)
        ctx!.save()
        ctx!.strokeStyle = '#FFB23D'
        ctx!.lineWidth = 2
        ctx!.beginPath()
        ctx!.arc(x, y, pulse, 0, Math.PI * 2)
        ctx!.stroke()
        ctx!.restore()
      }

      if (pctRef.current) {
        const total = pathData.coordinates.length - 1
        const pct = total > 0 ? Math.min(100, Math.round((stateRef.current.headPos / total) * 100)) : 100
        pctRef.current.textContent = `${pct}% CLEARED`
      }
    }

    const total = pathData.coordinates.length - 1
    const speed = total / LOOP_SECONDS

    function tick(ts: number) {
      const st = stateRef.current
      if (st.lastTs === 0) st.lastTs = ts
      const dt = Math.min(0.05, (ts - st.lastTs) / 1000)
      st.lastTs = ts

      if (!highlightRef.current) {
        if (st.fading) {
          const { w, h } = sizeRef.current
          const fctx = trailCanvasRef.current?.getContext('2d')
          if (fctx) {
            fctx.fillStyle = 'rgba(7, 11, 18, 0.08)'
            fctx.fillRect(0, 0, w, h)
          }
          if (ts - st.fadeStart > FADE_SECONDS * 1000) {
            trailCanvasRef.current?.getContext('2d')?.clearRect(0, 0, w, h)
            st.fading = false
            st.headPos = 0
          }
        } else {
          const prev = st.headPos
          st.headPos = Math.min(total, st.headPos + speed * dt)
          if (st.headPos > prev) addTrail(prev, st.headPos)
          if (st.headPos >= total) {
            st.fading = true
            st.fadeStart = ts
          }
        }
      }

      drawFrame()
      rafRef.current = requestAnimationFrame(tick)
    }

    if (reduceMotion) {
      stateRef.current.headPos = total
      if (total > 0) addTrail(0, total)
      drawFrame()
    } else {
      rafRef.current = requestAnimationFrame(tick)
    }

    return () => {
      ro.disconnect()
      if (rafRef.current) cancelAnimationFrame(rafRef.current)
      stateRef.current = { headPos: 0, lastTs: 0, fading: false, fadeStart: 0 }
    }
  }, [pathData])

  return (
    <div ref={screenRef} className={styles.screen}>
      <canvas ref={canvasRef} className={styles.canvas} />
      <div className={styles.crt} />
      <div className={styles.hud}>{t('map.title')}</div>
      <span ref={pctRef} className={styles.pct}>0% CLEARED</span>
      <div className={styles.legend}>
        <span className={styles.legendItem}>
          <span className={styles.swatch} style={{ background: '#33E07A' }} />
          {t('map.covered')}
        </span>
        <span className={styles.legendItem}>
          <span className={styles.swatch} style={{ background: 'rgba(255,74,60,0.6)' }} />
          {t('map.uncovered')}
        </span>
      </div>
    </div>
  )
}
