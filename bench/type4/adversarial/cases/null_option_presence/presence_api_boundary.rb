class Box
  def nil?
    true
  end
end

def rb_redefined(value, other)
  value.nil?
end

def value.nil?
  true
end

def rb_singleton_redefined(value, other)
  value.nil?
end

def nil?
  true
end

def rb_top_level_redefined(value, other)
  value.nil?
end

class Object
  alias nil? object_id
end

def rb_alias_redefined(value, other)
  value.nil?
end

class Object
  undef_method :nil?
end

def rb_undef_redefined(value, other)
  value.nil?
end

define_method(:nil?) do
  true
end

def rb_define_method_redefined(value, other)
  value.nil?
end

def rb_define_singleton_redefined(value, other)
  value.define_singleton_method(:nil?) { true }
  value.nil?
end
