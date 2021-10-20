import v8 from 'v8';

const structuredClone = obj => {
  return v8.deserialize(v8.serialize(obj));
};

const deepClone = (obj /*, name, depth = 0, tree = ''*/ ) => {
  /*if (depth > deep) {
    deep = depth;
    console.log1('Depth: ', depth, name, obj.constructor.name);
  }
  if (depth > 20) throw new Error(tree);*/
  let clone = structuredClone(obj);
  Object.setPrototypeOf(clone, obj.constructor.prototype);
  // console.log1('depth: ', depth);

  for (let name of Object.getOwnPropertyNames(obj)) {
    if (clone[name] instanceof Array || (clone[name] && clone[name].constructor != obj[name].constructor)) {
      clone[name] = deepClone(obj[name] /*, name, depth + 1, tree + ' => ' + name*/ );
    }
  }

  return clone;
}

export default deepClone;