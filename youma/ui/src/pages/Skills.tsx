import { useEffect } from 'react';
import { useNavigate } from 'react-router-dom';

export default function SkillsPage() {
  const navigate = useNavigate();
  useEffect(() => { navigate('/settings', { replace: true }); }, [navigate]);
  return null;
}
